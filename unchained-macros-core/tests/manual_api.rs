//! Handwritten native dispatch: no binding, detour, or listener proc macros.

use std::{
    num::NonZeroUsize,
    sync::atomic::{AtomicUsize, Ordering},
};

use unchained_macros_core::*;

struct Backend;

impl AddressResolver for Backend {
    type Context = ();

    unsafe fn resolve(&self, _: &()) -> Result<NonZeroUsize, String> {
        Ok(NonZeroUsize::new(original as *const () as usize).unwrap())
    }
}

static RESOLVER: Resolver<Backend> = Resolver {
    name: "manual",
    implementation: Backend,
    filter: None,
};

static BINDING: FunctionBinding<Backend> = FunctionBinding::new("manual", &RESOLVER);

static ALIAS: FunctionBinding<Backend> = FunctionBinding::new("late alias", &RESOLVER);

type Descriptor = NativeTarget<unsafe extern "C" fn(usize) -> usize, unsafe fn(&usize), unsafe fn(&usize), Backend>;

static TARGET: Descriptor = unsafe { Descriptor::new(&BINDING, enter, exit, |f| f as *const ()) };

static ENTRIES: AtomicUsize = AtomicUsize::new(0);

static EXITS: AtomicUsize = AtomicUsize::new(0);

static CONDITIONS: AtomicUsize = AtomicUsize::new(0);

inventory::submit! { &BINDING as &dyn __private::FunctionInfo }

inventory::submit! { &ALIAS as &dyn __private::FunctionInfo }

#[inline(never)]
unsafe extern "C" fn original(value: usize) -> usize {
    std::hint::black_box(value).wrapping_add(1)
}

unsafe fn call_original(value: usize) -> usize {
    let call: unsafe extern "C" fn(usize) -> usize =
        unsafe { std::mem::transmute(TARGET.binding().address().unwrap().get()) };

    unsafe { call(value) }
}

unsafe extern "C" fn enter(value: usize) -> usize {
    TARGET.observe_entry(|callback| unsafe { callback(&value) });
    unsafe { call_original(value) }
}

unsafe extern "C" fn exit(value: usize) -> usize {
    let result = unsafe { call_original(value) };
    TARGET.observe_exit(|callback| unsafe { callback(&result) });
    result
}

unsafe fn entered(_: &usize) {
    ENTRIES.fetch_add(1, Ordering::SeqCst);
}

unsafe fn returned(_result: &usize) {
    EXITS.fetch_add(1, Ordering::SeqCst);
}

unsafe extern "C" fn replacement(value: usize) -> usize {
    unsafe { call_original(value) + 10 }
}

fn gate() -> bool {
    CONDITIONS.fetch_add(1, Ordering::SeqCst);

    true
}

static ENTRY: ListenerRegistration = ListenerRegistration::new("entry", &ListenerAction::entry(&TARGET, entered), gate);

static EXIT: ListenerRegistration = ListenerRegistration::new("exit", &ListenerAction::exit(&TARGET, returned), gate);

static DUPLICATE: ListenerRegistration =
    ListenerRegistration::new("duplicate entry", &ListenerAction::entry(&TARGET, entered), gate);

static DETOUR: DetourRegistration<Descriptor> = DetourRegistration::new("detour", &TARGET, replacement, gate);

inventory::submit! { &ENTRY }

inventory::submit! { &EXIT }

inventory::submit! { &DUPLICATE }

// Direct attachment does not require submitting the detour registration.

#[test]

fn handwritten_registration_owns_controls_and_preserves_original_bypass() {
    assert!(binding_inventory_for::<Backend>().any(|binding| std::ptr::eq(binding, &BINDING)));

    assert!(std::ptr::eq(DETOUR.target().binding(), &BINDING));

    assert_eq!(ENTRY.target_identity(), DUPLICATE.target_identity());

    assert_eq!(ENTRY.target_identity(), EXIT.target_identity());

    unsafe {
        assert!(ENTRY.attach().is_err());

        assert!(!ENTRY.is_attached());

        BINDING
            .initialize_address(NonZeroUsize::new(original as *const () as usize).unwrap())
            .unwrap();

        ENTRY.attach().unwrap();

        assert_eq!(CONDITIONS.load(Ordering::SeqCst), 2);

        assert!(ENTRY.is_attached());

        assert!(EXIT.attach().is_err());

        assert!(DETOUR.attach().is_err());

        assert!(!DETOUR.is_attached());

        assert!(DUPLICATE.attach().is_err());

        assert!(!DUPLICATE.is_attached());

        let call = std::hint::black_box(original as unsafe extern "C" fn(usize) -> usize);

        assert_eq!(call(4), 5);

        assert_eq!(ENTRIES.load(Ordering::SeqCst), 1);

        assert_eq!(EXITS.load(Ordering::SeqCst), 0);

        assert_eq!(call_original(4), 5);

        assert_eq!(ENTRIES.load(Ordering::SeqCst), 1);

        ALIAS.initialize_address(BINDING.entry_address().unwrap()).unwrap();

        assert_eq!(ALIAS.address(), BINDING.address());

        assert_eq!(call(4), 5);

        assert_eq!(EXITS.load(Ordering::SeqCst), 0);
    }
}

static VALUE: ValueBinding = ValueBinding::new("manual pointer slot");

static SKIPPED: ValueBinding = ValueBinding::new("skipped required value");

static GLOBAL: NativeGlobal<usize> = NativeGlobal::pointer_slot(&VALUE);

inventory::submit! { &VALUE }

inventory::submit! { &SKIPPED }

#[test]

fn required_inventory_references_accessor_storage_and_reads_current_slot() {
    assert!(inventory::iter::<&ValueBinding>
        .into_iter()
        .any(|value| std::ptr::eq(*value, &VALUE)));

    assert!(is_required_value(VALUE.name()));

    let first = Box::into_raw(Box::new(1_usize));

    let second = Box::into_raw(Box::new(2_usize));

    let slot = Box::leak(Box::new(first));

    let offsets = [(VALUE.name().to_string(), 0)].into_iter().collect();

    unsafe {
        initialize_value_bindings_where((slot as *mut *mut usize) as usize, &offsets, |name| {
            name == VALUE.name()
        });

        assert!(SKIPPED.get().is_none());

        assert_eq!(GLOBAL.read(), first);

        *slot = second;

        assert_eq!(GLOBAL.read(), second);
    }
}
