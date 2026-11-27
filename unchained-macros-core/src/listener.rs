//! Typed observation slots. Backends supply attachment and invoke these slots.

use crate::{AddressResolver, Resolver};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

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

    fn snapshot(&self) -> T {
        *self
    }

    fn borrow(snapshot: &T) -> &T {
        snapshot
    }
}

thread_local! {
    static OBSERVING: std::cell::RefCell<ObservationStack> =
        const { std::cell::RefCell::new(ObservationStack { targets: [0; 64], depth: 0 }) };
}

// Fixed storage avoids recursively allocating while observing allocator calls.
struct ObservationStack {
    targets: [usize; 64],
    depth: usize,
}

struct ObservationGuard(usize);

impl ObservationGuard {
    fn enter(target: usize) -> Option<Self> {
        OBSERVING
            .try_with(|active| {
                let mut active = active.try_borrow_mut().ok()?;

                if active.depth == active.targets.len() || active.targets[..active.depth].contains(&target) {
                    return None;
                }

                let depth = active.depth;

                active.targets[depth] = target;

                active.depth += 1;

                Some(Self(target))
            })
            .ok()
            .flatten()
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
    pub const fn new() -> Self {
        Self {
            value: OnceLock::new(),
            enabled: AtomicBool::new(false),
        }
    }

    pub fn is_registered(&self) -> bool {
        self.value.get().is_some()
    }

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
    /// slot.register(123_usize, true).unwrap();
    /// ```

    pub unsafe fn register(&self, value: T, enabled: bool) -> Result<(), String> {
        if self.is_registered() {
            return Err("Only one callback of each kind is supported per method".into());
        }

        self.value
            .set(value)
            .map_err(|_| "Only one callback of each kind is supported per method".to_string())?;

        self.set_enabled(enabled);

        Ok(())
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
    }

    pub fn get(&self) -> Option<T> {
        self.enabled
            .load(Ordering::Acquire)
            .then(|| self.value.get().copied())
            .flatten()
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

    pub unsafe fn from_snapshot<S: 'static>(snapshot: S, finish: unsafe fn(*mut (), *const ())) -> Self {
        unsafe fn release<S>(state: *mut ()) {
            unsafe {
                drop(Box::from_raw(state.cast::<S>()));
            }
        }

        Self {
            state: Box::into_raw(Box::new(snapshot)).cast(),
            finish,
            release: release::<S>,
        }
    }

    /// # Safety
    /// Result storage must match the typed descriptor and remain live during this call.

    pub unsafe fn finish(self, result: *const ()) {
        isolate_observer(|| unsafe { (self.finish)(self.state, result) });
    }
}

impl Drop for ExitObservation {
    fn drop(&mut self) {
        isolate_observer(|| unsafe { (self.release)(self.state) });
    }
}

/// Attachment, resolver identity, and the shared guard for all callbacks on a target.
/// Each method needs distinct, process-lifetime target storage. Its resolver and
/// callback slots must be stable across discovery, attachment, and dispatch.
///
/// Owned arguments and narrowed borrow lifetimes cannot be registered:
/// ```compile_fail
/// use unchained_macros_core::*;
/// unsafe fn owns(_: String) {}
/// fn invalid(
///     descriptor: &'static impl SupportsEntry<Entry = unsafe fn(&String)>,
/// ) {
///     unsafe { descriptor.register_entry(owns, true, || true).unwrap(); }
/// }
/// ```
/// ```compile_fail
/// use unchained_macros_core::*;
/// unsafe fn retains(_: &'static u32) {}
/// fn invalid(
///     descriptor: &'static impl SupportsEntry<Entry = unsafe fn(&u32)>,
/// ) {
///     unsafe { descriptor.register_entry(retains, true, || true).unwrap(); }
/// }
/// ```
/// Exit result types are checked independently of inputs:
/// ```compile_fail
/// use unchained_macros_core::*;
/// unsafe fn wrong_result(_: &u32, _: &u64) {}
/// unsafe fn prepare(_: &u32) -> ExitObservation { unreachable!() }
/// fn invalid(
///     descriptor: &'static impl SupportsExit<
///         Exit = unsafe fn(&u32, &u32), Prepare = unsafe fn(&u32) -> ExitObservation,
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
/// # struct Native;
/// # #[runtime_bindings] impl Native { #[bind_resolved_func(Backend, &RESOLVER)] unsafe fn Call(value: String) {} }
/// #[bind_func_exit_listener(Native::Call)]
/// unsafe fn cannot_snapshot(value: &String, result: &()) {}
/// ```

pub trait CallbackTarget: Sync + 'static {
    fn resolver(&self) -> &'static dyn crate::resolver::ResolverInfo;

    /// # Safety
    /// Satisfy backend quiescence and readiness requirements.

    unsafe fn ensure_attached(&'static self) -> Result<(), Box<dyn std::error::Error>>;

    fn activation(&self, activate: bool, condition: fn() -> bool) -> bool {
        activate && observe(std::ptr::from_ref(self).cast::<()>() as usize, condition).unwrap_or(false)
    }
}

pub trait SupportsEntry: CallbackTarget {
    type Entry: Copy + Send + Sync + 'static;

    fn entry_slot(&self) -> &CallbackSlot<Self::Entry>;

    fn observe_entry<T>(&self, invoke: impl FnOnce(Self::Entry) -> T) -> Option<T> {
        observe(std::ptr::from_ref(self).cast::<()>() as usize, || {
            self.entry_slot().get().map(invoke)
        })
        .flatten()
    }

    /// # Safety
    /// The backend's quiescence/readiness requirements must hold.

    unsafe fn register_entry(
        &'static self,
        callback: Self::Entry,
        activate: bool,
        condition: fn() -> bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.entry_slot().is_registered() {
            return Err("Only one entry listener is supported per method".into());
        }

        unsafe {
            self.ensure_attached()?;
        }

        unsafe {
            self.entry_slot()
                .register(callback, self.activation(activate, condition))?;
        }

        Ok(())
    }
}

pub trait SupportsExit: CallbackTarget {
    type Exit: Copy + Sync + 'static;

    type Prepare: Copy + Send + Sync + 'static;

    fn exit_slot(&self) -> &CallbackSlot<Self::Prepare>;

    fn prepare_exit(&self, prepare: impl FnOnce(Self::Prepare) -> ExitObservation) -> Option<ExitObservation> {
        observe(std::ptr::from_ref(self).cast::<()>() as usize, || {
            self.exit_slot().get().map(prepare)
        })
        .flatten()
    }

    /// # Safety
    /// Result storage must match the target and remain live during the callback.

    unsafe fn finish_exit(&self, observation: ExitObservation, result: *const ()) {
        observe(std::ptr::from_ref(self).cast::<()>() as usize, || unsafe {
            observation.finish(result)
        });
    }

    /// # Safety
    /// `prepare` must own safe snapshots and deliver the matching inputs/result to `callback`.
    /// Satisfy target attachment requirements.

    unsafe fn register_exit(
        &'static self,
        _callback: Self::Exit,
        prepare: Self::Prepare,
        activate: bool,
        condition: fn() -> bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.exit_slot().is_registered() {
            return Err("Only one exit listener is supported per method".into());
        }

        unsafe {
            self.ensure_attached()?;
        }

        unsafe {
            self.exit_slot()
                .register(prepare, self.activation(activate, condition))?;
        }

        Ok(())
    }
}

// These immutable tuples can be promoted in a static without naming the target's
// concrete type. Both attachment and inspection use the same target and phase.
trait ListenerAction: Sync {
    fn target(&self) -> &'static dyn CallbackTarget;

    fn enabled_flag(&self) -> &AtomicBool;

    unsafe fn install(&self, activate: bool, condition: fn() -> bool) -> Result<(), Box<dyn std::error::Error>>;
}

impl<T: SupportsEntry> ListenerAction for (&'static T, T::Entry) {
    fn target(&self) -> &'static dyn CallbackTarget {
        self.0
    }

    fn enabled_flag(&self) -> &AtomicBool {
        &self.0.entry_slot().enabled
    }

    unsafe fn install(&self, activate: bool, condition: fn() -> bool) -> Result<(), Box<dyn std::error::Error>> {
        unsafe { self.0.register_entry(self.1, activate, condition) }
    }
}

impl<T: SupportsExit> ListenerAction for (&'static T, T::Exit, T::Prepare) {
    fn target(&self) -> &'static dyn CallbackTarget {
        self.0
    }

    fn enabled_flag(&self) -> &AtomicBool {
        &self.0.exit_slot().enabled
    }

    unsafe fn install(&self, activate: bool, condition: fn() -> bool) -> Result<(), Box<dyn std::error::Error>> {
        unsafe { self.0.register_exit(self.1, self.2, activate, condition) }
    }
}

/// Process-lifetime listener ownership. Attach only during quiescent startup.

pub struct ListenerRegistration {
    pub name: &'static str,
    action: &'static dyn ListenerAction,
    condition: fn() -> bool,
    initialized: AtomicBool,
}

impl ListenerRegistration {
    pub const fn entry<T: SupportsEntry>(
        name: &'static str,
        action: &'static (&'static T, T::Entry),
        condition: fn() -> bool,
    ) -> Self {
        Self {
            name,
            action,
            condition,
            initialized: AtomicBool::new(false),
        }
    }

    pub const fn exit<T: SupportsExit>(
        name: &'static str,
        action: &'static (&'static T, T::Exit, T::Prepare),
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
    /// Callers must be quiescent. Callbacks (including exit preparation) must
    /// uphold the target's ABI and ownership contracts. Do not attach concurrently.

    pub unsafe fn attach(&self, activate: bool) -> Result<(), Box<dyn std::error::Error>> {
        if self.initialized() {
            return Err("Listener callback is already initialized".into());
        }

        unsafe {
            self.action.install(activate, self.condition)?;
        }

        self.initialized.store(true, Ordering::Release);

        Ok(())
    }

    pub fn initialized(&self) -> bool {
        self.initialized.load(Ordering::Acquire)
    }

    pub fn enabled(&self) -> bool {
        self.initialized() && self.action.enabled_flag().load(Ordering::Acquire)
    }

    pub fn set_enabled(&self, enabled: bool) {
        if self.initialized() {
            self.action.enabled_flag().store(enabled, Ordering::Release);
        }
    }

    pub fn resolver(&self) -> &'static dyn crate::resolver::ResolverInfo {
        self.action.target().resolver()
    }

    pub fn condition(&self) -> bool {
        self.action.target().activation(true, self.condition)
    }

    pub fn slot_identity(&self) -> *const () {
        std::ptr::from_ref(self.action.enabled_flag()).cast()
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
/// Slot storage identity remains distinct even when functions are linker-folded.

pub fn validate_listener_registrations_for<R: AddressResolver>(
    snapshot: &crate::SelectorSnapshot,
) -> Result<(), String> {
    let mut slots = std::collections::BTreeMap::new();

    listener_inventory_for::<R>()
        .filter(|listener| listener.resolver().supports(snapshot))
        .try_for_each(
            |listener| match slots.insert(listener.slot_identity() as usize, listener.name) {
                None => Ok(()),
                Some(previous) => Err(format!(
                    "Only one listener per method phase is supported: {previous} and {} both register for {}",
                    listener.name,
                    listener.resolver().name(),
                )),
            },
        )
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
