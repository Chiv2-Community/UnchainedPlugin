//! Typed observation slots. Backends supply attachment and invoke these slots.

use crate::{AddressResolver, Resolver};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// One installed callback.
pub struct CallbackSlot<T> {
    value: OnceLock<T>,
}

impl<T: Copy> CallbackSlot<T> {
    pub const fn new() -> Self {
        Self { value: OnceLock::new() }
    }

    pub fn is_registered(&self) -> bool {
        self.value.get().is_some()
    }

    /// Publish a low-level callback without descriptor signature/attachment checks.
    ///
    /// # Safety
    /// The callback and result ABI must match the containing descriptor.
    /// Complete its attachment/readiness contract first. Any condition that can
    /// reenter native code must terminate any resulting callback recursion.
    ///
    /// Safe code cannot bypass descriptor validation through a public slot:
    /// ```compile_fail
    /// use unchained_macros_core::CallbackSlot;
    /// let slot = CallbackSlot::new();
    /// slot.register(123_usize).unwrap();
    /// ```
    pub unsafe fn register(&self, value: T) -> Result<(), String> {
        self.value
            .set(value)
            .map_err(|_| "Only one callback per method is supported".to_string())?;

        Ok(())
    }

    pub fn get(&self) -> Option<T> {
        self.value.get().copied()
    }
}

impl<T: Copy> Default for CallbackSlot<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Catch Rust observer/condition panics; native exceptions and aborts are not caught.
pub fn isolate_observer<T>(callback: impl FnOnce() -> T) -> Option<T> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(callback)) {
        Ok(value) => Some(value),
        Err(payload) => {
            // Do not invoke a logger here: it can panic or reenter a native hook.
            // Panic payload destructors are user code too.
            if let Err(nested) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(payload))) {
                std::mem::forget(nested);
            }

            None
        }
    }
}

/// Attachment and resolver identity for a target.
/// Each method needs distinct, process-lifetime target storage. Its resolver and
/// callback slot must be stable across discovery, attachment, and dispatch.
///
/// Owned arguments and narrowed borrow lifetimes cannot be registered:
/// ```compile_fail
/// use unchained_macros_core::*;
/// unsafe fn owns(_: String) {}
/// fn invalid(
///     descriptor: &'static impl SupportsEntry<Entry = unsafe fn(&String)>,
/// ) {
///     unsafe { descriptor.register(Callback::Entry(owns)).unwrap(); }
/// }
/// ```
/// ```compile_fail
/// use unchained_macros_core::*;
/// unsafe fn retains(_: &'static u32) {}
/// fn invalid(
///     descriptor: &'static impl SupportsEntry<Entry = unsafe fn(&u32)>,
/// ) {
///     unsafe { descriptor.register(Callback::Entry(retains)).unwrap(); }
/// }
/// ```
/// Exit listeners borrow only the original result:
/// ```compile_fail
/// use unchained_macros_core::*;
/// unsafe fn wrong_result(_: &u64) {}
/// fn invalid(target: &'static impl SupportsExit<Exit = unsafe fn(&u32)>) {
///     unsafe { target.register(Callback::Exit(wrong_result)).unwrap(); }
/// }
/// ```
pub trait CallbackTarget: Sized + Sync + 'static {
    type Callback: Copy + Send + Sync + 'static;
    type Entry: Copy + Send + Sync + 'static;
    type Exit: Copy + Send + Sync + 'static;

    fn slot(&self) -> &CallbackSlot<Callback<Self::Callback, Self::Entry, Self::Exit>>;
    fn resolver(&self) -> &'static dyn crate::resolver::ResolverInfo;
    fn physical_detour(&self) -> Option<&retour::RawDetour>;

    /// # Safety
    /// Satisfy backend quiescence and readiness requirements.
    unsafe fn install_hook(
        &'static self,
        callback: Callback<Self::Callback, Self::Entry, Self::Exit>,
    ) -> Result<(), Box<dyn std::error::Error>>;

    /// # Safety
    /// Attach during quiescent startup with the target's verified ABI.
    unsafe fn register(
        &'static self,
        callback: Callback<Self::Callback, Self::Entry, Self::Exit>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.slot().is_registered() {
            return Err("Only one callback per method is supported".into());
        }
        unsafe {
            self.install_hook(callback)?;
        }
        unsafe {
            self.slot().register(callback)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub enum Callback<F, E, X> {
    Detour(F),
    Entry(E),
    Exit(X),
}

pub trait SupportsEntry: CallbackTarget {
    fn observe_entry<T>(&self, invoke: impl FnOnce(Self::Entry) -> T) -> Option<T> {
        let Callback::Entry(callback) = self.slot().get()? else {
            return None;
        };
        isolate_observer(|| invoke(callback))
    }
}

pub trait SupportsExit: CallbackTarget {
    fn observe_exit<T>(&self, invoke: impl FnOnce(Self::Exit) -> T) -> Option<T> {
        let Callback::Exit(callback) = self.slot().get()? else {
            return None;
        };
        isolate_observer(|| invoke(callback))
    }
}

trait ListenerInfo: Sync {
    fn resolver(&self) -> &'static dyn crate::resolver::ResolverInfo;
    fn identity(&self) -> *const ();

    fn enabled(&self) -> bool;

    unsafe fn install(&self) -> Result<(), Box<dyn std::error::Error>>;
}

pub struct ListenerAction<T: CallbackTarget> {
    target: &'static T,
    callback: Callback<T::Callback, T::Entry, T::Exit>,
}

impl<T: SupportsEntry> ListenerAction<T> {
    pub const fn entry(target: &'static T, callback: T::Entry) -> Self {
        Self {
            target,
            callback: Callback::Entry(callback),
        }
    }
}

impl<T: SupportsExit> ListenerAction<T> {
    pub const fn exit(target: &'static T, callback: T::Exit) -> Self {
        Self {
            target,
            callback: Callback::Exit(callback),
        }
    }
}

impl<T: CallbackTarget> ListenerInfo for ListenerAction<T> {
    fn resolver(&self) -> &'static dyn crate::resolver::ResolverInfo {
        self.target.resolver()
    }
    fn identity(&self) -> *const () {
        std::ptr::from_ref(self.target).cast()
    }

    fn enabled(&self) -> bool {
        self.target.physical_detour().is_none_or(|hook| hook.is_enabled()) && self.target.slot().get().is_some()
    }

    unsafe fn install(&self) -> Result<(), Box<dyn std::error::Error>> {
        unsafe { self.target.register(self.callback) }
    }
}

/// Process-lifetime listener ownership. Attach only during quiescent startup.
pub struct ListenerRegistration {
    pub name: &'static str,
    action: &'static dyn ListenerInfo,
    condition: fn() -> bool,
    initialized: AtomicBool,
}

impl ListenerRegistration {
    pub const fn new<T: CallbackTarget>(
        name: &'static str,
        action: &'static ListenerAction<T>,
        condition: fn() -> bool,
    ) -> Self {
        Self {
            name,
            action,
            condition,
            initialized: AtomicBool::new(false),
        }
    }

    /// # Safety
    /// Callers must be quiescent. Callbacks must
    /// uphold the target's ABI and ownership contracts. Do not attach concurrently.
    pub unsafe fn attach(&self) -> Result<bool, Box<dyn std::error::Error>> {
        if self.initialized() {
            return Err("Listener callback is already initialized".into());
        }

        if !isolate_observer(self.condition).unwrap_or(false) {
            return Ok(false);
        }
        unsafe {
            self.action.install()?;
        }

        self.initialized.store(true, Ordering::Release);

        Ok(true)
    }

    pub fn initialized(&self) -> bool {
        self.initialized.load(Ordering::Acquire)
    }

    pub fn enabled(&self) -> bool {
        self.initialized() && self.action.enabled()
    }

    pub fn resolver(&self) -> &'static dyn crate::resolver::ResolverInfo {
        self.action.resolver()
    }

    pub fn target_identity(&self) -> *const () {
        self.action.identity()
    }
}

inventory::collect!(&'static ListenerRegistration);

pub fn listener_is_initialized(name: &str) -> bool {
    inventory::iter::<&'static ListenerRegistration>
        .into_iter()
        .any(|listener| listener.name == name && listener.initialized())
}

pub fn listener_is_enabled(name: &str) -> bool {
    inventory::iter::<&'static ListenerRegistration>
        .into_iter()
        .any(|listener| listener.name == name && listener.enabled())
}

pub fn listener_inventory_for<R: AddressResolver>() -> impl Iterator<Item = &'static ListenerRegistration> {
    inventory::iter::<&'static ListenerRegistration>
        .into_iter()
        .copied()
        .filter(|entry| (entry.resolver() as &dyn std::any::Any).is::<Resolver<R>>())
}

/// Reject excess registrations before any physical attachment or slot publication.
/// Target identity remains distinct even when functions are linker-folded.
pub fn validate_callback_registrations_for<R: AddressResolver>(
    snapshot: &crate::SelectorSnapshot,
) -> Result<(), String> {
    let mut slots = std::collections::BTreeMap::new();

    let listeners = listener_inventory_for::<R>()
        .filter(|listener| listener.resolver().supports(snapshot))
        .map(|l| (l.target_identity(), l.name, l.resolver().name()));
    let detours = crate::detour_inventory_for::<R>()
        .filter(|d| d.resolver().supports(snapshot))
        .map(|d| (d.target_identity(), d.name(), d.resolver().name()));
    for (target, name, resolver) in listeners.chain(detours) {
        if let Some(previous) = slots.insert(target, name) {
            return Err(format!(
                "Only one callback per method is supported: {previous} and {name} both register for {resolver}"
            ));
        }
    }
    Ok(())
}
