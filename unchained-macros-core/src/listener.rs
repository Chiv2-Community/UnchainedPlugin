//! Typed observation slots. Backends supply attachment and invoke these slots.
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::{AddressResolver, Resolver};

pub type ListenerAttach = unsafe fn() -> Result<(), Box<dyn std::error::Error>>;

/// Own entry-time input storage until exit observation completes.
///
/// # Safety
/// Snapshot creation must preserve the observed value without stealing native
/// ownership. `borrow` must reference storage owned/retained by the snapshot,
/// valid until it is dropped. Reachable native allocations must remain valid too.
/// Dropping the snapshot must release only its independent ownership and must
/// not panic, including during partial snapshot preparation.
///
/// Borrowed snapshots cannot escape through the associated owner:
/// ```compile_fail
/// use unchained_macros_core::ObserverSnapshot;
/// struct Borrowed<'a>(&'a str);
/// unsafe impl<'a> ObserverSnapshot for Borrowed<'a> {
///     type Snapshot = Borrowed<'a>;
///     fn snapshot(&self) -> Self::Snapshot { Borrowed(self.0) }
///     fn borrow(snapshot: &Self::Snapshot) -> &Self { snapshot }
/// }
/// ```
pub unsafe trait ObserverSnapshot: Sized {
    type Snapshot: 'static;
    fn snapshot(&self) -> Self::Snapshot;
    fn borrow(snapshot: &Self::Snapshot) -> &Self;
}

// Copy snapshots preserve values, not the lifetime of pointees/native resources.
unsafe impl<T: Copy + 'static> ObserverSnapshot for T {
    type Snapshot = T;
    fn snapshot(&self) -> T { *self }
    fn borrow(snapshot: &T) -> &T { snapshot }
}

thread_local! {
    static OBSERVING: std::cell::RefCell<ObservationStack> =
        const { std::cell::RefCell::new(ObservationStack { targets: [0; 64], depth: 0 }) };
}

// Fixed storage avoids recursively allocating while observing allocator calls.
struct ObservationStack { targets: [usize; 64], depth: usize }
struct ObservationGuard(usize);
impl ObservationGuard {
    fn enter(target: usize) -> Option<Self> {
        OBSERVING.try_with(|active| {
            let mut active = active.try_borrow_mut().ok()?;
            if active.depth == active.targets.len() || active.targets[..active.depth].contains(&target) {
                return None;
            }
            let depth = active.depth;
            active.targets[depth] = target;
            active.depth += 1;
            Some(Self(target))
        }).ok().flatten()
    }
}
impl Drop for ObservationGuard {
    fn drop(&mut self) {
        let _ = OBSERVING.try_with(|active| {
            let mut active = active.borrow_mut();
            active.depth -= 1;
            debug_assert_eq!(active.targets[active.depth], self.0);
        });
    }
}

fn observe<T>(target: usize, callback: impl FnOnce() -> T) -> Option<T> {
    let _guard = ObservationGuard::enter(target)?;
    isolate_observer(callback)
}

/// A callback's activation is independent of the shared physical hook.
pub struct CallbackSlot<T> {
    value: OnceLock<T>,
    enabled: AtomicBool,
}

impl<T: Copy> CallbackSlot<T> {
    pub const fn new() -> Self { Self { value: OnceLock::new(), enabled: AtomicBool::new(false) } }
    pub fn is_registered(&self) -> bool { self.value.get().is_some() }
    /// Publish a low-level callback without descriptor signature/attachment checks.
    ///
    /// # Safety
    /// The callback and snapshot/result ABI must match the containing descriptor.
    /// Complete its attachment/readiness contract first. Any condition that can
    /// reenter native code must be evaluated through its descriptor's `activation`.
    ///
    /// Safe code cannot bypass descriptor validation through a public slot:
    /// ```compile_fail
    /// use unchained_macros_core::CallbackSlot;
    /// let slot = CallbackSlot::new();
    /// slot.register(123_usize, true, || true).unwrap();
    /// ```
    pub unsafe fn register(&self, value: T, activate: bool, condition: fn() -> bool) -> Result<(), String> {
        if self.is_registered() { return Err("Only one callback of each kind is supported per method".into()); }
        let enabled = activate && isolate_observer(condition).unwrap_or(false);
        self.value.set(value)
            .map_err(|_| "Only one callback of each kind is supported per method".to_string())?;
        self.set_enabled(enabled);
        Ok(())
    }
    pub fn set_enabled(&self, enabled: bool) { self.enabled.store(enabled, Ordering::Release); }
    pub fn get(&self) -> Option<T> {
        self.enabled.load(Ordering::Acquire).then(|| self.value.get().copied()).flatten()
    }
}

impl<T: Copy> Default for CallbackSlot<T> {
    fn default() -> Self { Self::new() }
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

/// An owned input snapshot with backend-independent result delivery.
///
/// The backend must pass a pointer to live result storage of the descriptor's
/// declared result type. Dropping without finishing releases the snapshot.
pub struct ExitObservation {
    state: *mut (),
    finish: unsafe fn(*mut (), *const ()),
    release: unsafe fn(*mut ()),
}

impl ExitObservation {
    /// # Safety
    /// `finish` must borrow state as `S`, without freeing, moving it out, or
    /// retaining references. Result storage must match the descriptor's result.
    pub unsafe fn from_snapshot<S: 'static>(
        snapshot: S, finish: unsafe fn(*mut (), *const ()),
    ) -> Self {
        unsafe fn release<S>(state: *mut ()) {
            unsafe { drop(Box::from_raw(state.cast::<S>())); }
        }
        Self { state: Box::into_raw(Box::new(snapshot)).cast(), finish, release: release::<S> }
    }

    /// # Safety
    /// Result storage must match the typed descriptor and remain live during this call.
    pub unsafe fn finish(self, result: *const ()) {
        isolate_observer(|| unsafe { (self.finish)(self.state, result) });
    }
}

impl Drop for ExitObservation {
    fn drop(&mut self) { isolate_observer(|| unsafe { (self.release)(self.state) }); }
}

/// Per-method extension point for native or backend-specific observation.
///
/// `E` is the immutable entry callback, `X` the immutable exit callback, and `P`
/// the snapshot factory taking the same borrowed inputs as `E`. Signatures are
/// invariant: adapters must not narrow the callback's observation lifetimes.
///
/// Owned arguments and narrowed borrow lifetimes cannot be registered:
/// ```compile_fail
/// use unchained_macros_core::*;
/// unsafe fn owns(_: String) {}
/// fn invalid<R: AddressResolver>(
///     descriptor: &ListenerDescriptor<unsafe fn(&String), (), (), R>,
/// ) {
///     unsafe { descriptor.register_entry(owns, true, || true).unwrap(); }
/// }
/// ```
/// ```compile_fail
/// use unchained_macros_core::*;
/// unsafe fn retains(_: &'static u32) {}
/// fn invalid<R: AddressResolver>(
///     descriptor: &ListenerDescriptor<unsafe fn(&u32), (), (), R>,
/// ) {
///     unsafe { descriptor.register_entry(retains, true, || true).unwrap(); }
/// }
/// ```
/// Exit result types are checked independently of inputs:
/// ```compile_fail
/// use unchained_macros_core::*;
/// unsafe fn wrong_result(_: &u32, _: &u64) {}
/// unsafe fn prepare(_: &u32) -> ExitObservation { unreachable!() }
/// fn invalid<R: AddressResolver>(
///     descriptor: &ListenerDescriptor<
///         unsafe fn(&u32), unsafe fn(&u32, &u32),
///         unsafe fn(&u32) -> ExitObservation, R,
///     >,
/// ) {
///     unsafe { descriptor.register_exit(wrong_result, prepare, true, || true).unwrap(); }
/// }
/// ```
/// Inputs without an `ObserverSnapshot` implementation fail before attachment:
/// ```compile_fail
/// use unchained_macros_core::*;
/// # struct Backend;
/// # impl AddressResolver for Backend {
/// #     type Context = ();
/// #     unsafe fn resolve(&self, _: &()) -> Result<std::num::NonZeroUsize, String> { unreachable!() }
/// # }
/// # static RESOLVER: Resolver<Backend> = Resolver { name: "owned", implementation: Backend, filter: None };
/// # unsafe fn attach() -> Result<(), Box<dyn std::error::Error>> { Ok(()) }
/// # type Descriptor = ListenerDescriptor<unsafe fn(&String), unsafe fn(&String, &()), unsafe fn(&String) -> ExitObservation, Backend>;
/// # static DESCRIPTOR: Descriptor = unsafe { Descriptor::new(&RESOLVER, attach) };
/// # struct Native;
/// # impl Native {
/// #     const __UNCHAINED_LISTENERS_Call: &'static Descriptor = &DESCRIPTOR;
/// # }
/// #[bind_func_exit_listener(Native::Call)]
/// unsafe fn cannot_snapshot(value: &String, result: &()) {}
/// ```
pub struct ListenerDescriptor<E, X, P, R: AddressResolver> {
    pub resolver: &'static Resolver<R>,
    pub entry: CallbackSlot<E>,
    pub exit: CallbackSlot<P>,
    attach: ListenerAttach,
    signature: std::marker::PhantomData<fn(X) -> X>,
}

impl<E: Copy, X, P: Copy, R: AddressResolver> ListenerDescriptor<E, X, P, R> {
    /// # Safety
    /// Attachment and dispatch must honor E/X/P, preserve original execution,
    /// and finish snapshots only with the declared caller-visible result.
    pub const unsafe fn new(resolver: &'static Resolver<R>, attach: ListenerAttach) -> Self {
        Self { resolver, entry: CallbackSlot::new(), exit: CallbackSlot::new(), attach,
            signature: std::marker::PhantomData }
    }
    /// # Safety
    /// The backend's quiescence/readiness requirements must hold.
    pub unsafe fn ensure_attached(&self) -> Result<(), Box<dyn std::error::Error>> {
        unsafe { (self.attach)() }
    }
    /// Evaluate an attachment-time gate under the same observer guard as dispatch.
    /// Global auto-activation off skips evaluation entirely.
    pub fn activation(&self, activate: bool, condition: fn() -> bool) -> bool {
        activate && observe(self as *const Self as usize, condition).unwrap_or(false)
    }
    /// Invoke the active entry observer. Reentry into this method while an
    /// observer/condition runs suppresses observation, not native execution.
    pub fn observe_entry<T>(&self, invoke: impl FnOnce(E) -> T) -> Option<T> {
        observe(self as *const Self as usize, || self.entry.get().map(invoke)).flatten()
    }
    /// Snapshot inputs before execution. No slot/guard borrow survives this call.
    pub fn prepare_exit(&self, prepare: impl FnOnce(P) -> ExitObservation) -> Option<ExitObservation> {
        observe(self as *const Self as usize, || self.exit.get().map(prepare)).flatten()
    }
    /// Deliver the actual caller-visible result after normal execution.
    ///
    /// # Safety
    /// Result storage must match X and remain live during the callback.
    pub unsafe fn finish_exit(&self, observation: ExitObservation, result: *const ()) {
        observe(self as *const Self as usize, || unsafe { observation.finish(result) });
    }
    /// # Safety
    /// The backend's quiescence/readiness requirements must hold.
    pub unsafe fn register_entry(&self, callback: E, activate: bool, condition: fn() -> bool) -> Result<(), Box<dyn std::error::Error>> {
        if self.entry.is_registered() { return Err("Only one entry listener is supported per method".into()); }
        unsafe { (self.attach)()?; }
        let activate = self.activation(activate, condition);
        unsafe { self.entry.register(callback, activate, || true)?; }
        Ok(())
    }
    /// # Safety
    /// `prepare` must own safe snapshots and invoke `callback` with matching
    /// inputs/result. The backend's attachment requirements must hold.
    pub unsafe fn register_exit(&self, _callback: X, prepare: P, activate: bool, condition: fn() -> bool) -> Result<(), Box<dyn std::error::Error>> {
        if self.exit.is_registered() { return Err("Only one exit listener is supported per method".into()); }
        unsafe { (self.attach)()?; }
        let activate = self.activation(activate, condition);
        unsafe { self.exit.register(prepare, activate, || true)?; }
        Ok(())
    }
}

trait CallbackControl: Send + Sync {
    fn enabled(&self) -> bool;
    fn set_enabled(&self, enabled: bool);
    fn storage_identity(&self) -> *const ();
}
impl<T: Copy + Send + Sync + 'static> CallbackControl for CallbackSlot<T> {
    fn enabled(&self) -> bool { self.get().is_some() }
    fn set_enabled(&self, enabled: bool) { self.set_enabled(enabled); }
    fn storage_identity(&self) -> *const () { self as *const Self as *const () }
}

/// Process-lifetime listener ownership. Attach only during quiescent startup.
pub struct ListenerRegistration {
    pub name: &'static str,
    pub resolver: &'static dyn crate::resolver::ResolverInfo,
    install: crate::CreateHookFn,
    pub condition: fn() -> bool,
    slot: &'static dyn CallbackControl,
    initialized: AtomicBool,
}
impl ListenerRegistration {
    pub const fn new<R: AddressResolver, T: Copy + Send + Sync + 'static>(
        name: &'static str, resolver: &'static Resolver<R>, slot: &'static CallbackSlot<T>,
        install: crate::CreateHookFn, condition: fn() -> bool,
    ) -> Self {
        Self { name, resolver, slot, install, condition, initialized: AtomicBool::new(false) }
    }
    /// # Safety
    /// Callers must be quiescent. The installer must publish this slot's typed
    /// process-lifetime callback and reject duplicate ownership before publication.
    /// It must not publish the callback on failure.
    /// Do not attach concurrently.
    pub unsafe fn attach(&self, activate: bool) -> Result<(), Box<dyn std::error::Error>> {
        if self.initialized() { return Err("Listener callback is already initialized".into()); }
        unsafe { (self.install)(activate)?; }
        self.initialized.store(true, Ordering::Release);
        Ok(())
    }
    pub fn initialized(&self) -> bool { self.initialized.load(Ordering::Acquire) }
    pub fn enabled(&self) -> bool { self.initialized() && self.slot.enabled() }
    pub fn set_enabled(&self, enabled: bool) {
        if self.initialized() { self.slot.set_enabled(enabled); }
    }
    pub fn slot_identity(&self) -> *const () { self.slot.storage_identity() }
}

inventory::collect!(&'static ListenerRegistration);

pub fn listener_is_initialized(name: &str) -> bool {
    inventory::iter::<&'static ListenerRegistration>.into_iter()
        .any(|listener| listener.name == name && listener.initialized())
}
pub fn listener_is_enabled(name: &str) -> bool {
    inventory::iter::<&'static ListenerRegistration>.into_iter()
        .any(|listener| listener.name == name && listener.enabled())
}

pub fn listener_inventory_for<R: AddressResolver>() -> impl Iterator<Item = &'static ListenerRegistration> {
    inventory::iter::<&'static ListenerRegistration>.into_iter().copied()
        .filter(|entry| (entry.resolver as &dyn std::any::Any).is::<Resolver<R>>())
}

/// Reject excess registrations before any physical attachment or slot publication.
/// Slot storage identity remains distinct even when functions are linker-folded.
pub fn validate_listener_registrations_for<R: AddressResolver>(
    snapshot: &crate::SelectorSnapshot,
) -> Result<(), String> {
    let mut slots = std::collections::BTreeMap::new();
    for listener in listener_inventory_for::<R>().filter(|listener| listener.resolver.supports(snapshot)) {
        if let Some(previous) = slots.insert(listener.slot_identity() as usize, listener.name) {
            return Err(format!(
                "Only one listener per method phase is supported: {previous} and {} both register for {}",
                listener.name, listener.resolver.name(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observer_stack_is_bounded_and_recovers_without_holding_borrows() {
        fn nest(target: usize) -> usize {
            observe(target, || 1 + nest(target + 1)).unwrap_or(0)
        }
        assert_eq!(nest(1), 64);
        assert!(observe(1, || ()).is_some());
    }
}
