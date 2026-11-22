//! Observation of the real exec thunk. No frame or argument expression is replayed.

use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    marker::PhantomData,
    ptr::NonNull,
    rc::Rc,
};

/// A verified, non-ref/out parameter's completed VM result slot.
///
/// The slot belongs to the original thunk, not Rust. It may only be borrowed
/// during the entry callback; the thunk destroys its temporaries on return.
#[derive(Clone, Copy)]
pub struct Argument<'a> {
    pointer: NonNull<c_void>,
    _borrow: PhantomData<&'a c_void>,
    _game_thread: PhantomData<Rc<()>>,
}

impl Argument<'_> {
    /// # Safety
    /// `T` must match the validated VM slot layout. Do not move from the slot,
    /// retain the borrow, mutate native storage, or release native ownership.
    pub unsafe fn borrow<T>(&self) -> &T {
        assert!(self.pointer.as_ptr().cast::<T>().is_aligned(), "Misaligned VM argument slot");
        unsafe { &*self.pointer.as_ptr().cast::<T>() }
    }

    /// VM bool arguments occupy a 32-bit slot, unlike native bool returns.
    ///
    /// # Safety
    /// The validated parameter must be a VM boolean.
    pub unsafe fn boolean(&self) -> bool {
        assert!(self.pointer.as_ptr().cast::<u32>().is_aligned(), "Misaligned VM bool32 slot");
        unsafe { self.pointer.as_ptr().cast::<u32>().read() != 0 }
    }
}

/// Callback-scoped read-only access to argument slots.
pub struct Inputs<'a> {
    slots: &'a [NonNull<c_void>],
    _game_thread: PhantomData<Rc<()>>,
}

impl Inputs<'_> {
    pub fn len(&self) -> usize { self.slots.len() }
    pub fn is_empty(&self) -> bool { self.slots.is_empty() }
    pub fn get(&self, index: usize) -> Option<Argument<'_>> {
        self.slots.get(index).copied().map(|pointer| Argument {
            pointer, _borrow: PhantomData, _game_thread: PhantomData,
        })
    }
}

/// One entry callback; fan-out is deliberately not implemented by this backend.
pub type EntryCallback = for<'a> unsafe fn(Inputs<'a>);

/// Prepare independently owned entry snapshots. Native ownership must never be
/// bit-copied; pointer snapshots do not extend their pointees' lifetimes.
pub type SnapshotCallback = for<'a> unsafe fn(Inputs<'a>) -> Option<unchained_macros_core::ExitObservation>;
pub type FinishCallback = unsafe fn(unchained_macros_core::ExitObservation, *const c_void);

#[derive(Clone, Copy, Default)]
pub struct Callbacks {
    pub entry: Option<EntryCallback>,
    pub snapshot: Option<SnapshotCallback>,
    /// Backend-specific per-target observer guard for result delivery.
    pub finish: Option<FinishCallback>,
    /// Zero for untyped adapters; typed descriptors require their native alignment.
    pub result_alignment: usize,
}

fn report(message: &str) {
    // A logger may panic (including a panicking payload destructor) or reenter
    // the same failing VM hook. Neither may escape or recursively log forever.
    REPORTING.with(|active| {
        if active.replace(true) { return; }
        unchained_macros_core::isolate_observer(|| log::error!("{message}"));
        active.set(false);
    });
}

struct Invocation {
    frame: usize,
    expected: usize,
    /// Property identities, in step order; zero permits the bytecode Step path.
    properties: &'static [usize],
    depth: usize,
    slots: Vec<NonNull<c_void>>,
    completed: bool,
    failed: bool,
    callbacks: Callbacks,
    snapshot: Option<unchained_macros_core::ExitObservation>,
}

thread_local! {
    static CALLS: RefCell<Vec<Rc<RefCell<Invocation>>>> = const { RefCell::new(Vec::new()) };
    static REPORTING: Cell<bool> = const { Cell::new(false) };
}

struct Scope(Rc<RefCell<Invocation>>);
impl Drop for Scope {
    fn drop(&mut self) {
        let _ = CALLS.try_with(|calls| {
            let removed = calls.borrow_mut().pop();
            debug_assert!(removed.as_ref().is_some_and(|call| Rc::ptr_eq(call, &self.0)));
        });
    }
}

fn ready(call: &Rc<RefCell<Invocation>>) {
    let (callbacks, slots) = {
        let mut state = call.borrow_mut();
        if state.failed || state.completed || state.slots.len() != state.expected { return; }
        state.completed = true;
        (state.callbacks, state.slots.clone())
    };
    // No TLS/state borrow is held while executing arbitrary callback code.
    let inputs = || Inputs { slots: &slots, _game_thread: PhantomData };
    if let Some(entry) = callbacks.entry {
        if unchained_macros_core::isolate_observer(|| unsafe { entry(inputs()) }).is_none() {
            report("Kismet entry listener panicked");
        }
    }
    if let Some(snapshot) = callbacks.snapshot {
        match unchained_macros_core::isolate_observer(|| unsafe { snapshot(inputs()) }) {
            Some(snapshot) => call.borrow_mut().snapshot = snapshot,
            None => report("Kismet input snapshot panicked; exit listener suppressed"),
        }
    }
}

/// Run a verified exec thunk exactly once on the original frame.
///
/// # Safety
/// `properties` must describe exactly the thunk's top-level Step/ExplicitProperty
/// calls. Every result slot must remain live, initialized and immutable until
/// the final argument completes. Ref/out layouts are unsupported. All Step and
/// StepExplicitProperty calls must pass through [`observe_step`]. The original
/// must receive the real frame/context/result, with its original native ABI.
/// Callbacks must obey their slot/snapshot contracts and run on the game thread.
/// Exit callbacks must only interpret validated caller-owned result storage.
pub unsafe fn observe_exec(
    frame: *mut c_void,
    result: *mut c_void,
    properties: &'static [usize],
    callbacks: Callbacks,
    original: impl FnOnce(),
) {
    let call = Rc::new(RefCell::new(Invocation {
        frame: frame as usize, expected: properties.len(), properties,
        depth: 0, slots: Vec::with_capacity(properties.len()), completed: false,
        failed: false, callbacks, snapshot: None,
    }));
    if CALLS.try_with(|calls| calls.borrow_mut().push(call.clone())).is_err() {
        original();
        return;
    }
    let _scope = Scope(call.clone());
    ready(&call); // No-argument thunks have no Step completion.
    original();
    let (snapshot, valid) = {
        let mut state = call.borrow_mut();
        (state.snapshot.take(), state.completed && !state.failed)
    };
    if valid {
        if let Some(snapshot) = snapshot {
            if result.is_null() {
                report("Kismet exit observation has null caller result storage; callback suppressed");
                return;
            }
            let callbacks = call.borrow().callbacks;
            if callbacks.result_alignment != 0 && result as usize % callbacks.result_alignment != 0 {
                report("Kismet exit observation has misaligned result storage; callback suppressed");
                return;
            }
            if unchained_macros_core::isolate_observer(|| unsafe {
                let finish = callbacks.finish;
                if let Some(finish) = finish { finish(snapshot, result); }
                else { snapshot.finish(result.cast()); }
            }).is_none() {
                report("Kismet exit listener panicked");
            }
        }
    } else {
        report("Kismet exec observation did not match its verified step contract");
    }
}

struct StepScope {
    call: Rc<RefCell<Invocation>>,
}
impl Drop for StepScope {
    fn drop(&mut self) { self.call.borrow_mut().depth -= 1; }
}

/// Forward a real Step call and observe only its top-level completion.
///
/// # Safety
/// Forward all native arguments unchanged and invoke the original exactly once.
/// `property` is zero for Step, and the actual FProperty identity for
/// StepExplicitProperty. `result` is the original, initialized result slot after
/// the original returns. This function must cover recursive Step calls as well.
pub unsafe fn observe_step(
    frame: *mut c_void,
    result: *mut c_void,
    property: usize,
    original: impl FnOnce(),
) {
    let call = CALLS.try_with(|calls| calls.borrow().iter().rev()
        .find(|call| call.borrow().frame == frame as usize).cloned()).ok().flatten();
    let Some(call) = call else { original(); return; };
    let top_level = {
        let mut state = call.borrow_mut();
        state.depth += 1;
        state.depth == 1 && !state.completed && !state.failed
    };
    let scope = StepScope { call: call.clone() };
    original();
    drop(scope);
    if !top_level { return; }
    {
        let mut state = call.borrow_mut();
        let index = state.slots.len();
        let Some(pointer) = NonNull::new(result) else { state.failed = true; return; };
        if index >= state.expected || (property != 0 && state.properties[index] != property) {
            state.failed = true;
            return;
        }
        state.slots.push(pointer);
    }
    ready(&call);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    thread_local! {
        static EVENTS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
        static ENTRIES: Cell<usize> = const { Cell::new(0) };
    }
    fn event(name: &'static str) { EVENTS.with(|events| events.borrow_mut().push(name)); }
    unsafe fn entry(inputs: Inputs<'_>) {
        assert_eq!(unsafe { *inputs.get(0).unwrap().borrow::<u32>() }, 42);
        event("entry");
    }
    unsafe fn snapshot(inputs: Inputs<'_>) -> Option<unchained_macros_core::ExitObservation> {
        let value = Box::new(unsafe { *inputs.get(0).unwrap().borrow::<u32>() });
        Some(unsafe { unchained_macros_core::ExitObservation::new(
            Box::into_raw(value).cast(), exit, release,
        ) })
    }
    unsafe fn release(snapshot: *mut ()) { unsafe { drop(Box::from_raw(snapshot.cast::<u32>())); } }
    unsafe fn exit(snapshot: *mut (), result: *const ()) {
        assert_eq!(unsafe { *snapshot.cast::<u32>() }, 42);
        // Native bool result is one byte; no bool32 overread.
        assert_eq!(unsafe { result.cast::<u8>().read() }, 1);
        event("exit");
    }
    struct Cleanup;
    impl Drop for Cleanup { fn drop(&mut self) { event("cleanup"); } }

    #[test]
    fn original_real_frame_evaluation_body_and_cleanup_run_once() {
        EVENTS.with(|events| events.borrow_mut().clear());
        let mut frame = 0u32;
        let pointer = (&mut frame as *mut u32).cast();
        let mut result = 0u8;
        let result_pointer = (&mut result as *mut u8).cast();
        unsafe {
            observe_exec(pointer, result_pointer, &[11], Callbacks {
                entry: Some(entry), snapshot: Some(snapshot), ..Callbacks::default()
            }, || {
                let _cleanup = Cleanup;
                let mut argument = 0u32;
                let slot = (&mut argument as *mut u32).cast();
                observe_step(pointer, slot, 11, || {
                    event("evaluate");
                    frame += 1;
                    argument = 42;
                });
                event("body");
                assert_eq!(argument, 42);
                result = 1;
            });
        }
        assert_eq!(frame, 1);
        assert_eq!(result, 1);
        EVENTS.with(|events| assert_eq!(*events.borrow(), ["evaluate", "entry", "body", "cleanup", "exit"]));
    }

    #[test]
    fn recursive_same_frame_step_does_not_capture_nested_result() {
        EVENTS.with(|events| events.borrow_mut().clear());
        let mut frame = 0u8;
        let pointer = (&mut frame as *mut u8).cast();
        unsafe {
            observe_exec(pointer, std::ptr::null_mut(), &[11], Callbacks {
                entry: Some(entry), ..Callbacks::default()
            }, || {
                let mut outer = 0u32;
                let mut nested = 0u32;
                observe_step(pointer, (&mut outer as *mut u32).cast(), 0, || {
                    observe_step(pointer, (&mut nested as *mut u32).cast(), 0, || nested = 99);
                    outer = 42;
                });
            });
        }
        EVENTS.with(|events| assert_eq!(*events.borrow(), ["entry"]));
    }

    unsafe fn reenter(inputs: Inputs<'_>) {
        assert_eq!(inputs.len(), 1);
        ENTRIES.with(|entries| {
            let count = entries.get();
            entries.set(count + 1);
            if count == 0 {
                let mut frame = 0u8;
                let pointer = (&mut frame as *mut u8).cast();
                unsafe {
                    observe_exec(pointer, std::ptr::null_mut(), &[11], Callbacks {
                        entry: Some(reenter), ..Callbacks::default()
                    }, || {
                        let mut slot = 7u32;
                        observe_step(pointer, (&mut slot as *mut u32).cast(), 11, || {});
                    });
                }
            }
        });
    }

    #[test]
    fn listener_reentry_uses_independent_scope() {
        ENTRIES.with(|entries| entries.set(0));
        let mut frame = 0u8;
        let pointer = (&mut frame as *mut u8).cast();
        unsafe {
            observe_exec(pointer, std::ptr::null_mut(), &[11], Callbacks {
                entry: Some(reenter), ..Callbacks::default()
            }, || {
                let mut slot = 9u32;
                observe_step(pointer, (&mut slot as *mut u32).cast(), 11, || {});
            });
        }
        ENTRIES.with(|entries| assert_eq!(entries.get(), 2));
        CALLS.with(|calls| assert!(calls.borrow().is_empty()));
    }

    #[test]
    fn wrong_compiled_in_property_suppresses_callbacks_not_original() {
        EVENTS.with(|events| events.borrow_mut().clear());
        let mut frame = 0u8;
        let pointer = (&mut frame as *mut u8).cast();
        let mut evaluated = 0;
        unsafe {
            observe_exec(pointer, std::ptr::null_mut(), &[11], Callbacks {
                entry: Some(entry), ..Callbacks::default()
            }, || {
                let mut argument = 0u32;
                observe_step(pointer, (&mut argument as *mut u32).cast(), 12, || {
                    evaluated += 1;
                    argument = 42;
                });
                event("body");
            });
        }
        assert_eq!(evaluated, 1);
        EVENTS.with(|events| assert_eq!(*events.borrow(), ["body"]));
    }

    unsafe fn noargs(inputs: Inputs<'_>) { assert!(inputs.is_empty()); event("entry"); }
    #[test]
    fn no_arguments_need_no_step_and_keep_original() {
        EVENTS.with(|events| events.borrow_mut().clear());
        unsafe {
            observe_exec(std::ptr::dangling_mut(), std::ptr::null_mut(), &[], Callbacks {
                entry: Some(noargs), ..Callbacks::default()
            }, || event("body"));
        }
        EVENTS.with(|events| assert_eq!(*events.borrow(), ["entry", "body"]));
    }

    #[test]
    fn normal_return_only_exit_and_scope_cleanup_on_unwind() {
        let mut frame = 0u8;
        let pointer = (&mut frame as *mut u8).cast();
        let result = catch_unwind(AssertUnwindSafe(|| unsafe {
            observe_exec(pointer, std::ptr::null_mut(), &[], Callbacks::default(), || panic!("fixture"));
        }));
        assert!(result.is_err());
        CALLS.with(|calls| assert!(calls.borrow().is_empty()));
    }

    struct NativeOwner(String);
    impl Drop for NativeOwner { fn drop(&mut self) { event("owner-drop"); } }
    unsafe fn borrow_owner_then_panic(inputs: Inputs<'_>) {
        let argument = inputs.get(0).unwrap();
        let owner = unsafe { argument.borrow::<NativeOwner>() };
        assert_eq!(owner.0, "engine-owned");
        event("entry");
        panic!("isolated observer");
    }
    #[test]
    fn borrowed_owner_is_destroyed_only_by_original_after_panicking_listener() {
        EVENTS.with(|events| events.borrow_mut().clear());
        let mut frame = 0u8;
        let pointer = (&mut frame as *mut u8).cast();
        unsafe {
            observe_exec(pointer, std::ptr::null_mut(), &[11], Callbacks {
                entry: Some(borrow_owner_then_panic), ..Callbacks::default()
            }, || {
                // Construct a real owner; no zero-filled FString/FText surrogate.
                let mut owner = NativeOwner("engine-owned".into());
                observe_step(pointer, (&mut owner as *mut NativeOwner).cast(), 11, || {});
                event("body");
                assert_eq!(owner.0, "engine-owned");
            });
        }
        EVENTS.with(|events| assert_eq!(*events.borrow(), ["entry", "body", "owner-drop"]));
        CALLS.with(|calls| assert!(calls.borrow().is_empty()));
    }

    #[test]
    fn nested_exec_with_same_frame_has_its_own_argument_depth() {
        EVENTS.with(|events| events.borrow_mut().clear());
        let mut frame = 0u8;
        let pointer = (&mut frame as *mut u8).cast();
        unsafe {
            observe_exec(pointer, std::ptr::null_mut(), &[11], Callbacks {
                entry: Some(entry), ..Callbacks::default()
            }, || {
                let mut outer = 0u32;
                observe_step(pointer, (&mut outer as *mut u32).cast(), 0, || {
                    observe_exec(pointer, std::ptr::null_mut(), &[11], Callbacks {
                        entry: Some(entry), ..Callbacks::default()
                    }, || {
                        let mut inner = 0u32;
                        observe_step(pointer, (&mut inner as *mut u32).cast(), 11, || inner = 42);
                    });
                    outer = 42;
                });
            });
        }
        EVENTS.with(|events| assert_eq!(*events.borrow(), ["entry", "entry"]));
    }

    #[test]
    fn misaligned_argument_suppresses_callback_before_typed_borrow() {
        #[repr(align(4))]
        struct Buffer([u8; 8]);
        let mut buffer = Buffer([0; 8]);
        buffer.0[1..5].copy_from_slice(&42u32.to_ne_bytes());
        let slot = unsafe { buffer.0.as_mut_ptr().add(1).cast() };
        let frame = std::ptr::dangling_mut();
        EVENTS.with(|events| events.borrow_mut().clear());
        unsafe {
            observe_exec(frame, std::ptr::null_mut(), &[11], Callbacks {
                entry: Some(entry), ..Callbacks::default()
            }, || {
                observe_step(frame, slot, 11, || {});
                event("body");
            });
        }
        EVENTS.with(|events| assert_eq!(*events.borrow(), ["body"]));
    }
}
