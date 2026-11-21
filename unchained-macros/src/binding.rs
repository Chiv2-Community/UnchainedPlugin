use std::{num::NonZeroUsize, sync::OnceLock};

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

/// Address source for a native function.
pub enum FunctionSource {
    /// A scan root with its own selection policy.
    Resolver(&'static crate::ResolverDescriptor),
    /// A cache key populated by a separately required offset provider.
    ProviderOffset(&'static str),
}

/// Native target name, address source, and optional original-call trampoline.
pub struct FunctionBinding {
    name: &'static str,
    target: ResolvedAddress,
    detour: OnceLock<&'static retour::RawDetour>,
    source: FunctionSource,
}

impl FunctionBinding {
    pub const fn new(name: &'static str, resolver: &'static crate::ResolverDescriptor) -> Self {
        Self { name, target: ResolvedAddress::new(resolver.named.name), detour: OnceLock::new(), source: FunctionSource::Resolver(resolver) }
    }

    /// An address supplied by an offset provider, not a named resolver type.
    pub const fn from_offset_key(name: &'static str, key: &'static str) -> Self {
        Self { name, target: ResolvedAddress::new(key), detour: OnceLock::new(), source: FunctionSource::ProviderOffset(key) }
    }

    /// Native target name, independent of the resolver's cache key.
    pub const fn name(&self) -> &'static str { self.name }

    pub const fn source(&self) -> &FunctionSource { &self.source }

    pub const fn offset_key(&self) -> &'static str { self.target.name() }

    pub fn supports(&self, snapshot: &crate::SelectorSnapshot) -> bool {
        match self.source {
            FunctionSource::Resolver(resolver) => resolver.supports(snapshot),
            FunctionSource::ProviderOffset(_) => true,
        }
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

    /// Initialize once from the loaded module and its resolved relative offset.
    /// Repeating the same address is harmless; rebinding is rejected.
    ///
    /// # Safety
    /// For bindings used by native callers, the resolved address must identify
    /// process-lifetime code with their exact ABI and behavioral contract.
    /// Initialize every alias before attaching a detour to that native address.
    pub unsafe fn initialize(&self, base: usize, offset: u64) -> Result<(), String> {
        unsafe { self.target.try_initialize(base, offset) }
    }
}

/// Typed metadata emitted beside a native binding, not a callable Rust wrapper.
/// Invariance prevents replacing a higher-ranked native signature with a
/// narrower lifetime contract.
///
/// ```compile_fail
/// use unchained_macros::NativeFunction;
/// fn wrong_arguments(binding: NativeFunction<unsafe extern "C" fn(u32)>) {
///     let _: NativeFunction<unsafe extern "C" fn(u64)> = binding;
/// }
/// ```
///
/// ```compile_fail
/// use unchained_macros::NativeFunction;
/// fn narrowed_lifetime(binding: NativeFunction<for<'a> unsafe extern "C" fn(&'a i32) -> &'a i32>) {
///     let _: NativeFunction<unsafe extern "C" fn(&'static i32) -> &'static i32> = binding;
/// }
/// ```
#[doc(hidden)]
pub struct NativeFunction<F> {
    binding: &'static FunctionBinding,
    signature: std::marker::PhantomData<fn(F) -> F>,
}

/// Invariant signature equality witness; carries no callable address or contract.
#[doc(hidden)]
pub type NativeSignature<F> = std::marker::PhantomData<fn(F) -> F>;

impl<F> NativeFunction<F> {
    /// # Safety
    /// Every address published by `binding` must have the native signature `F`.
    pub const unsafe fn new(binding: &'static FunctionBinding) -> Self {
        Self { binding, signature: std::marker::PhantomData }
    }

    pub const fn binding(&self) -> &'static FunctionBinding { self.binding }
    pub const fn name(&self) -> &'static str { self.binding.name() }
}

/// References stable binding storage without duplicating its address or name.
pub struct BindingRegistration<T: 'static> {
    pub binding: &'static T,
}

pub type FunctionBindingRegistration = BindingRegistration<FunctionBinding>;
inventory::collect!(FunctionBindingRegistration);

/// Initialize registered functions from image-relative offsets.
///
/// # Safety
/// Each RVA must identify process-lifetime native code with its binding's ABI
/// and behavioral contract. Initialize before native calls or detour attachment.
pub unsafe fn initialize_function_bindings(
    base: usize,
    offsets: &std::collections::HashMap<String, u64>,
) -> Result<(), String> {
    unsafe { initialize_function_bindings_where(base, offsets, |_| true) }
}

/// Initialize only bindings accepted by the host's applicability predicate.
/// Rejected bindings remain unbound, even if cached offsets exist.
///
/// # Safety
/// The contracts of [`initialize_function_bindings`] apply to accepted bindings.
pub unsafe fn initialize_function_bindings_where(
    base: usize,
    offsets: &std::collections::HashMap<String, u64>,
    enabled: impl Fn(&FunctionBinding) -> bool,
) -> Result<(), String> {
    for registration in inventory::iter::<FunctionBindingRegistration>.into_iter()
        .filter(|registration| enabled(registration.binding))
    {
        let binding = registration.binding;
        let offset = offsets.get(binding.offset_key())
            .ok_or_else(|| format!("No address found for native binding {} (key {})", binding.name(), binding.offset_key()))?;
        unsafe { binding.initialize(base, *offset)?; }
    }
    Ok(())
}

/// Query publication by resolver/provider offset key, not detour name.
pub fn function_is_bound(name: &str) -> bool {
    inventory::iter::<FunctionBindingRegistration>.into_iter()
        .any(|registration| registration.binding.offset_key() == name && registration.binding.address().is_some())
}

/// Preserve fallible helper behavior before native binding initialization.
pub fn require_function_bindings(names: &[&str]) -> Result<(), String> {
    for name in names {
        if !function_is_bound(name) {
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
pub unsafe fn publish_function_detours(entry: usize, detour: &'static retour::RawDetour) -> Result<(), String> {
    for registration in inventory::iter::<FunctionBindingRegistration> {
        let binding = registration.binding;
        if binding.entry_address().is_some_and(|address| address.get() == entry) {
            unsafe { binding.publish_detour(detour)?; }
        }
    }
    Ok(())
}

/// Required native data address. Binding failures are fatal; generated access reads
/// slot contents or return an inline object's address without copying it.
pub type ValueBinding = ResolvedAddress;

/// Opt-in native types with generated static method forwarding.
pub trait NativeGlobalTarget {
    type Global;
}

/// Address metadata only; never caches a native pointer slot's contents.
#[doc(hidden)]
pub struct NativeGlobalPointer {
    binding: &'static ValueBinding,
    indirect: bool,
}

impl NativeGlobalPointer {
    pub const fn new(binding: &'static ValueBinding, indirect: bool) -> Self {
        Self { binding, indirect }
    }

    /// # Safety
    /// The native slot/object must be live and correctly typed. Slot reads must
    /// not race with native writes. One-time binding publication does
    /// not make ordinary C++ pointer writes atomic.
    pub unsafe fn as_ptr<T>(&self) -> *mut T {
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

pub type ValueBindingRegistration = BindingRegistration<ValueBinding>;

inventory::collect!(ValueBindingRegistration);

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
    for registration in inventory::iter::<ValueBindingRegistration>.into_iter()
        .filter(|registration| enabled(registration.binding.name()))
    {
        let binding = registration.binding;
        let offset = offsets.get(binding.name()).unwrap_or_else(|| {
            panic!("Required native value {} could not be resolved; no offset is available", binding.name())
        });
        unsafe { binding.initialize(base, *offset); }
    }
}

pub fn is_required_value(name: &str) -> bool {
    inventory::iter::<ValueBindingRegistration>.into_iter()
        .any(|registration| registration.binding.name() == name)
}
