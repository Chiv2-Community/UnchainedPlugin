use std::{num::NonZeroUsize, sync::OnceLock};
use std::{collections::BTreeMap, sync::Mutex};

// Shared across resolver implementations and initialization stages.
static ATTACHED_DETOURS: Mutex<BTreeMap<usize, &'static retour::RawDetour>> = Mutex::new(BTreeMap::new());

/// One-time publication of a resolved module address, shared by code and data.
pub struct ResolvedAddress {
    name: &'static str,
    address: OnceLock<NonZeroUsize>,
}

impl ResolvedAddress {
    pub const fn new(name: &'static str) -> Self {
        Self { name, address: OnceLock::new() }
    }

    pub const fn name(&self) -> &'static str { self.name }
    pub fn get(&self) -> Option<NonZeroUsize> { self.address.get().copied() }

    /// # Safety
    /// The address must identify process-lifetime storage/code with the caller's
    /// native contract. Publish before use; a different address is rejected.
    pub unsafe fn try_initialize(&self, base: usize, offset: u64) -> Result<(), String> {
        let address = usize::try_from(offset).ok()
            .and_then(|offset| base.checked_add(offset))
            .and_then(NonZeroUsize::new)
            .filter(|_| base != 0)
            .ok_or_else(|| format!("Invalid address for native binding {}", self.name))?;
        self.publish(address)
    }

    fn publish(&self, address: NonZeroUsize) -> Result<(), String> {
        match self.address.set(address) {
            Ok(()) => Ok(()),
            Err(address) if self.get() == Some(address) => Ok(()),
            Err(_) => Err(format!("Native binding {} is already initialized at another address", self.name)),
        }
    }

    pub fn address(&self) -> usize {
        self.get().unwrap_or_else(|| {
            panic!("Required native value {} is not bound; startup must resolve it before access", self.name)
        }).get()
    }

    /// Bind required data storage without reading or caching its contents.
    ///
    /// # Safety
    /// The resolved slot must remain readable with the declared layout.
    /// Native writes must be synchronized with reads.
    pub unsafe fn initialize(&self, base: usize, offset: u64) {
        unsafe { self.try_initialize(base, offset) }.unwrap_or_else(|error| {
            panic!("Failed to bind required native value {}: {error}", self.name)
        });
    }
}

/// Native target name, address source, and optional original-call trampoline.
pub struct FunctionBinding<R: crate::AddressResolver> {
    name: &'static str,
    target: ResolvedAddress,
    detour: OnceLock<&'static retour::RawDetour>,
    pub resolver: &'static crate::Resolver<R>,
}

impl<R: crate::AddressResolver> FunctionBinding<R> {
    pub const fn new(name: &'static str, resolver: &'static crate::Resolver<R>) -> Self {
        Self { name, target: ResolvedAddress::new(resolver.name), detour: OnceLock::new(), resolver }
    }

    /// Native target name, independent of the resolver's cache key.
    pub const fn name(&self) -> &'static str { self.name }

    pub const fn resolver_name(&self) -> &'static str { self.resolver.name }

    pub fn supports(&self, snapshot: &crate::SelectorSnapshot) -> bool {
        self.resolver.supports(snapshot)
    }

    /// Original-call address, bypassing this target's registered replacement.
    pub fn address(&self) -> Option<NonZeroUsize> {
        self.detour.get()
            .and_then(|detour| NonZeroUsize::new(detour.trampoline() as *const () as usize))
            .or_else(|| self.entry_address())
    }

    /// Resolved entry to patch, never the Rust binding wrapper or trampoline.
    pub fn entry_address(&self) -> Option<NonZeroUsize> {
        self.target.get()
    }

    /// Publish before enabling the patch; keep the trampoline alive permanently.
    ///
    /// # Safety
    /// The detour must target this entry with this binding's exact native
    /// contract. Attachment requires quiescent callers: publishing does not stop
    /// another thread that already loaded the old entry address.
    pub unsafe fn publish_detour(&self, detour: &'static retour::RawDetour) -> Result<(), String> {
        if self.entry_address().is_none() {
            return Err(format!("Native binding {} is not initialized", self.name()));
        }
        self.detour.set(detour)
            .map_err(|_| format!("Native binding {} already has a detour", self.name()))
    }

    /// Publish a verified native address. Repeating it is harmless; rebinding is rejected.
    ///
    /// # Safety
    /// For bindings used by native callers, the resolved address must identify
    /// process-lifetime code with their exact ABI and behavioral contract.
    /// Keep native callers quiescent until initialization and attachment complete.
    /// Late aliases inherit any trampoline already published for this address.
    pub unsafe fn initialize_address(&self, address: NonZeroUsize) -> Result<(), String> {
        let attached = ATTACHED_DETOURS.lock().map_err(|_| "Detour publication lock poisoned")?;
        self.target.publish(address)?;
        if let Some(detour) = attached.get(&address.get()) {
            if let Some(current) = self.detour.get() {
                if !std::ptr::eq(*current, *detour) { return Err("Binding has a different published trampoline".into()); }
            } else {
                unsafe { self.publish_detour(detour)?; }
            }
        }
        Ok(())
    }
}

/// Invariant signature equality witness; carries no callable address or contract.
#[doc(hidden)]
pub type NativeSignature<F> = std::marker::PhantomData<fn(F) -> F>;

#[doc(hidden)]
pub trait FunctionInfo: std::any::Any + Send + Sync {
    fn resolver(&self) -> &'static dyn crate::resolver::ResolverInfo;
    fn entry_address(&self) -> Option<NonZeroUsize>;
    unsafe fn publish_detour(&self, detour: &'static retour::RawDetour) -> Result<(), String>;
}
impl<R: crate::AddressResolver> FunctionInfo for FunctionBinding<R> {
    fn resolver(&self) -> &'static dyn crate::resolver::ResolverInfo { self.resolver }
    fn entry_address(&self) -> Option<NonZeroUsize> { self.entry_address() }
    unsafe fn publish_detour(&self, detour: &'static retour::RawDetour) -> Result<(), String> {
        unsafe { self.publish_detour(detour) }
    }
}
inventory::collect!(&'static dyn FunctionInfo);
pub fn binding_inventory_for<R: crate::AddressResolver>() -> impl Iterator<Item = &'static FunctionBinding<R>> {
    inventory::iter::<&'static dyn FunctionInfo>.into_iter().filter_map(|entry| (*entry as &dyn std::any::Any).downcast_ref())
}

/// Resolve and publish selected function bindings for one backend.
///
/// # Safety
/// Each address must identify process-lifetime native code with its binding's ABI
/// and behavioral contract. Meet the backend context's threading/readiness contract.
/// Initialize before native calls or detour attachment.
pub unsafe fn initialize_bindings_for<R: crate::AddressResolver>(
    context: &R::Context, snapshot: &crate::SelectorSnapshot,
) -> Result<(), String> {
    snapshot.validate_registered()?;
    let mut resolved = Vec::new();
    for binding in binding_inventory_for::<R>()
        .filter(|binding| binding.supports(snapshot))
    {
        let address = unsafe { binding.resolver.implementation.resolve(context) }
            .map_err(|error| format!("Binding {} ({}): {error}", binding.name(), binding.resolver_name()))?;
        resolved.push((binding, address));
    }
    for (binding, address) in resolved {
        unsafe { binding.initialize_address(address)?; }
    }
    Ok(())
}

pub fn function_is_bound_for<R: crate::AddressResolver>(name: &str) -> bool {
    binding_inventory_for::<R>().any(|entry| entry.resolver_name() == name && entry.address().is_some())
}

/// Preserve fallible helper behavior before native binding initialization.
pub fn require_function_bindings_for<R: crate::AddressResolver>(names: &[&str]) -> Result<(), String> {
    for name in names {
        if !function_is_bound_for::<R>(name) {
            return Err(format!("Native binding {name} is not initialized"));
        }
    }
    Ok(())
}

/// Share Retour's detour with every initialized alias of its entry.
///
/// # Safety
/// Call during quiescent startup before enabling the patch. The trampoline must
/// satisfy every binding's native contract and remain live for the process.
#[doc(hidden)]
pub(crate) unsafe fn publish_function_detours<R: crate::AddressResolver>(
    owner: &FunctionBinding<R>, detour: &'static retour::RawDetour,
) -> Result<(), String> {
    let entry = owner.entry_address().ok_or("Detour owner has no entry address")?;
    let mut attached = ATTACHED_DETOURS.lock().map_err(|_| "Detour publication lock poisoned")?;
    if attached.get(&entry.get()).is_some_and(|existing| !std::ptr::eq(*existing, detour)) {
        return Err(format!("A different trampoline is already published for {entry:#x}"));
    }
    // Manual targets need an original-call trampoline even without inventory.
    unsafe { owner.publish_detour(detour)?; }
    attached.insert(entry.get(), detour);
    for binding in inventory::iter::<&'static dyn FunctionInfo> {
        let same_storage = std::ptr::addr_eq(*binding as *const dyn FunctionInfo, owner as *const _);
        if !same_storage && binding.entry_address() == Some(entry) {
            unsafe { binding.publish_detour(detour)?; }
        }
    }
    Ok(())
}

/// Required native data address. Binding failures are fatal; generated access reads
/// slot contents or return an inline object's address without copying it.
pub type ValueBinding = ResolvedAddress;

/// Address metadata only; never caches a native pointer slot's contents.
pub struct NativeGlobal<T> {
    binding: &'static ValueBinding,
    indirect: bool,
    marker: std::marker::PhantomData<fn() -> T>,
}

impl<T> NativeGlobal<T> {
    /// Construct metadata for a slot holding a replaceable native pointer.
    pub const fn pointer_slot(binding: &'static ValueBinding) -> Self {
        Self { binding, indirect: true, marker: std::marker::PhantomData }
    }

    /// Construct metadata for inline native storage without copying it.
    pub const fn inline(binding: &'static ValueBinding) -> Self {
        Self { binding, indirect: false, marker: std::marker::PhantomData }
    }

    /// Read the current pointer from a `Value` slot, or return an `Address` object's pointer.
    /// Does not borrow native storage or dispatch methods.
    ///
    /// # Safety
    /// The native slot/object must be live and correctly typed. Slot reads must
    /// not race with native writes. One-time binding publication does
    /// not make ordinary C++ pointer writes atomic.
    /// The caller must uphold native lifetime and aliasing rules while using the pointer.
    pub unsafe fn read(&self) -> *mut T {
        let address = self.binding.address();
        let pointer = if self.indirect {
            unsafe { (address as *const *mut T).read() }
        } else {
            address as *mut T
        };
        assert!(!pointer.is_null(), "Required native value {} is null; the engine must initialize it before access", self.binding.name());
        pointer
    }
}

inventory::collect!(&'static ValueBinding);

/// Bind every required native value from cached or scanned module-relative offsets.
///
/// # Safety
/// Each resolved slot must satisfy its generated accessor's type/lifetime and
/// synchronization contract. This binds addresses, not their current contents.
pub unsafe fn initialize_value_bindings(base: usize, offsets: &std::collections::HashMap<String, u64>) {
    unsafe { initialize_value_bindings_where(base, offsets, |_| true); }
}

/// Initialize only values accepted by the host's applicability predicate.
/// Rejected values remain unbound, even if cached offsets exist.
///
/// # Safety
/// The contracts of [`initialize_value_bindings`] apply to accepted bindings.
pub unsafe fn initialize_value_bindings_where(
    base: usize,
    offsets: &std::collections::HashMap<String, u64>,
    enabled: impl Fn(&str) -> bool,
) {
    for binding in inventory::iter::<&'static ValueBinding>.into_iter()
        .filter(|binding| enabled(binding.name()))
    {
        let offset = offsets.get(binding.name()).unwrap_or_else(|| {
            panic!("Required native value {} could not be resolved; no offset is available", binding.name())
        });
        unsafe { binding.initialize(base, *offset); }
    }
}

pub fn is_required_value(name: &str) -> bool {
    inventory::iter::<&'static ValueBinding>.into_iter()
        .any(|binding| binding.name() == name)
}
