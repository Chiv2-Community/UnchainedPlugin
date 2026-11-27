//! Handwritten native dispatch: no binding, detour, or listener proc macros.
use std::{num::NonZeroUsize, sync::atomic::{AtomicUsize, Ordering}};
use unchained_macros_core::*;

struct Backend;
impl AddressResolver for Backend {
    type Context = ();
    unsafe fn resolve(&self, _: &()) -> Result<NonZeroUsize, String> {
        Ok(NonZeroUsize::new(original as *const () as usize).unwrap())
    }
}
static RESOLVER: Resolver<Backend> = Resolver { name: "manual", implementation: Backend, filter: None };
static BINDING: FunctionBinding<Backend> = FunctionBinding::new("manual", &RESOLVER);
static ALIAS: FunctionBinding<Backend> = FunctionBinding::new("late alias", &RESOLVER);
type Descriptor = NativeTarget<unsafe extern "C" fn(usize) -> usize,
    unsafe fn(&usize), unsafe fn(&usize, &usize), unsafe fn(&usize) -> ExitObservation, Backend,
>;
static TARGET: Descriptor = unsafe { Descriptor::new(&BINDING, dispatch, |f| f as *const ()) };
static ENTRIES: AtomicUsize = AtomicUsize::new(0);
static EXITS: AtomicUsize = AtomicUsize::new(0);
static CONDITIONS: AtomicUsize = AtomicUsize::new(0);

inventory::submit! { &BINDING as &dyn __private::FunctionInfo }
inventory::submit! { &ALIAS as &dyn __private::FunctionInfo }

#[inline(never)]
unsafe extern "C" fn original(value: usize) -> usize { std::hint::black_box(value).wrapping_add(1) }

unsafe fn call_original(value: usize) -> usize {
    let call: unsafe extern "C" fn(usize) -> usize =
        unsafe { std::mem::transmute(TARGET.binding().address().unwrap().get()) };
    unsafe { call(value) }
}
unsafe extern "C" fn dispatch(value: usize) -> usize {
    TARGET.observe_entry(|callback| unsafe { callback(&value) });
    let snapshot = TARGET.prepare_exit(|prepare| unsafe { prepare(&value) });
    let call: unsafe extern "C" fn(usize) -> usize =
        TARGET.replacement_slot().get()
            .unwrap_or_else(|| unsafe { std::mem::transmute(BINDING.address().unwrap().get()) });
    let result = unsafe { call(value) };
    if let Some(snapshot) = snapshot {
        unsafe { TARGET.finish_exit(snapshot, (&result as *const usize).cast()); }
    }
    result
}
unsafe fn entered(_: &usize) { ENTRIES.fetch_add(1, Ordering::SeqCst); }
unsafe fn returned(value: &usize, result: &usize) {
    assert!(*result == value + 1 || *result == value + 11);
    EXITS.fetch_add(1, Ordering::SeqCst);
}
unsafe fn finish(state: *mut (), result: *const ()) {
    unsafe { returned(&*state.cast::<usize>(), &*result.cast::<usize>()); }
}
unsafe fn prepare(value: &usize) -> ExitObservation {
    unsafe { ExitObservation::from_snapshot(*value, finish) }
}
unsafe extern "C" fn replacement(value: usize) -> usize { unsafe { call_original(value) + 10 } }
fn gate() -> bool { CONDITIONS.fetch_add(1, Ordering::SeqCst); true }

static ENTRY: ListenerRegistration = ListenerRegistration::entry("entry", &(&TARGET, entered), gate);
static EXIT: ListenerRegistration = ListenerRegistration::exit("exit", &(&TARGET, returned, prepare), gate);
static DUPLICATE: ListenerRegistration = ListenerRegistration::entry("duplicate entry", &(&TARGET, entered), gate);
static DETOUR: DetourRegistration<Descriptor> = DetourRegistration::new("detour", &TARGET, replacement, gate);
inventory::submit! { &ENTRY }
inventory::submit! { &EXIT }
inventory::submit! { &DUPLICATE }
// Direct attachment does not require submitting the detour registration.

#[test]
fn handwritten_registration_owns_controls_and_preserves_original_bypass() {
    assert!(binding_inventory_for::<Backend>().any(|binding| std::ptr::eq(binding, &BINDING)));
    assert!(std::ptr::eq(DETOUR.target().binding(), &BINDING));
    assert_eq!(ENTRY.slot_identity(), DUPLICATE.slot_identity());
    assert_ne!(ENTRY.slot_identity(), EXIT.slot_identity());
    unsafe {
        assert!(ENTRY.attach(true).is_err());
        assert!(!ENTRY.initialized());
        ENTRY.set_enabled(true);
        assert!(!ENTRY.enabled());
        BINDING.initialize_address(NonZeroUsize::new(original as *const () as usize).unwrap()).unwrap();
        ENTRY.attach(false).unwrap();
        assert_eq!(CONDITIONS.load(Ordering::SeqCst), 0);
        assert!(ENTRY.initialized());
        assert!(!ENTRY.enabled());
        ENTRY.set_enabled(true);
        EXIT.attach(true).unwrap();
        DETOUR.attach(true).unwrap();
        assert!(DETOUR.initialized());
        assert!(DETOUR.enabled());
        assert!(DUPLICATE.attach(true).is_err());
        assert!(!DUPLICATE.initialized());
        DUPLICATE.set_enabled(false);
        assert!(ENTRY.enabled());
        ENTRY.set_enabled(false);
        DUPLICATE.set_enabled(true);
        assert!(!ENTRY.enabled());
        ENTRY.set_enabled(true);
        let call = std::hint::black_box(original as unsafe extern "C" fn(usize) -> usize);
        assert_eq!(call(4), 15);
        assert_eq!(ENTRIES.load(Ordering::SeqCst), 1);
        assert_eq!(EXITS.load(Ordering::SeqCst), 1);
        assert_eq!(call_original(4), 5);
        assert_eq!(ENTRIES.load(Ordering::SeqCst), 1);
        ALIAS.initialize_address(BINDING.entry_address().unwrap()).unwrap();
        assert_eq!(ALIAS.address(), BINDING.address());
        DETOUR.set_enabled(false);
        assert!(!DETOUR.enabled());
        assert_eq!(call(4), 5);
        assert_eq!(EXITS.load(Ordering::SeqCst), 2);
        assert!(DETOUR.detour().unwrap().is_enabled());
        DETOUR.set_enabled(true);
        DETOUR.detour().unwrap().disable().unwrap();
        assert!(!DETOUR.enabled());
        assert!(DETOUR.initialized());
    }
}

static VALUE: ValueBinding = ValueBinding::new("manual pointer slot");
static SKIPPED: ValueBinding = ValueBinding::new("skipped required value");
static GLOBAL: NativeGlobal<usize> = NativeGlobal::pointer_slot(&VALUE);
inventory::submit! { &VALUE }
inventory::submit! { &SKIPPED }

#[test]
fn required_inventory_references_accessor_storage_and_reads_current_slot() {
    assert!(inventory::iter::<&ValueBinding>.into_iter().any(|value| std::ptr::eq(*value, &VALUE)));
    assert!(is_required_value(VALUE.name()));
    let first = Box::into_raw(Box::new(1_usize));
    let second = Box::into_raw(Box::new(2_usize));
    let slot = Box::leak(Box::new(first));
    let offsets = [(VALUE.name().to_string(), 0)].into_iter().collect();
    unsafe {
        initialize_value_bindings_where((slot as *mut *mut usize) as usize, &offsets, |name| name == VALUE.name());
        assert!(SKIPPED.get().is_none());
        assert_eq!(GLOBAL.read(), first);
        *slot = second;
        assert_eq!(GLOBAL.read(), second);
    }
}
