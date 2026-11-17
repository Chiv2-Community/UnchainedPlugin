use std::sync::atomic::{AtomicI32, AtomicPtr, Ordering};
use unchained_macros::{bind_resolver_value, initialize_value_binding, initialize_value_bindings, ValueBinding};

mod resolvers {
    pub use unchained_macros::{ValueBinding, ValueBindingRegistration};
    pub struct Count;
    pub struct Allocator;
    pub struct Objects;
}

static COUNT: AtomicI32 = AtomicI32::new(0);
static POINTER: AtomicPtr<i32> = AtomicPtr::new(std::ptr::null_mut());
static FIRST: i32 = 11;
static SECOND: i32 = 22;

// Deliberately non-Copy, with zero initial bytes where a pointer slot would be.
struct InlineObject(AtomicI32);
static OBJECTS: InlineObject = InlineObject(AtomicI32::new(0));

#[bind_resolver_value(resolvers::Objects, Address)]
unsafe fn objects() -> *const InlineObject {}

#[bind_resolver_value(resolvers::Count, Value)]
unsafe fn count() -> i32 {}

#[bind_resolver_value(resolvers::Allocator, Value)]
unsafe fn pointer() -> *const i32 {}

#[test]
fn required_values_bind_slots_read_current_contents_and_fail_loudly() {
    // No invocation may silently succeed before startup binds its slot.
    let failure = std::panic::catch_unwind(|| unsafe { count() }).unwrap_err();
    assert!(panic_message(failure).contains("Required native value Count is not bound"));
    let failure = std::panic::catch_unwind(|| unsafe { objects() }).unwrap_err();
    assert!(panic_message(failure).contains("Required native value Objects is not bound"));

    let failure = std::panic::catch_unwind(|| unsafe {
        initialize_value_bindings(1, &std::collections::HashMap::new());
    }).unwrap_err();
    assert!(panic_message(failure).contains("could not be resolved; no offset is available"));

    let offsets = std::collections::HashMap::from([
        ("Count".into(), (COUNT.as_ptr() as usize - 1) as u64),
        ("Allocator".into(), (POINTER.as_ptr() as usize - 1) as u64),
        ("Objects".into(), ((&raw const OBJECTS) as usize - 1) as u64),
    ]);
    unsafe {
        // Binding a slot before the engine populates it is permitted.
        initialize_value_bindings(1, &offsets);
        initialize_value_binding("Count", COUNT.as_ptr() as usize);
        assert_eq!(count(), 0); // Zero scalar values are valid.
        COUNT.store(-42, Ordering::Relaxed);
        assert_eq!(count(), -42);
        assert_eq!(objects(), &raw const OBJECTS);
        assert_eq!((*objects()).0.load(Ordering::Relaxed), 0);
        OBJECTS.0.store(77, Ordering::Relaxed);
        assert_eq!(objects(), &raw const OBJECTS);
        assert_eq!((*objects()).0.load(Ordering::Relaxed), 77);
        initialize_value_binding("Objects", (&raw const OBJECTS) as usize);
        assert_eq!(objects(), &raw const OBJECTS);
    }

    let failure = std::panic::catch_unwind(|| unsafe { pointer() }).unwrap_err();
    assert!(panic_message(failure).contains("Required native value Allocator is null"));
    POINTER.store((&raw const FIRST).cast_mut(), Ordering::Relaxed);
    assert_eq!(unsafe { pointer() }, &raw const FIRST);
    POINTER.store((&raw const SECOND).cast_mut(), Ordering::Relaxed);
    assert_eq!(unsafe { pointer() }, &raw const SECOND);
}

fn panic_message(error: Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = error.downcast_ref::<String>() { return message.clone(); }
    error.downcast_ref::<&str>().expect("string panic").to_string()
}

#[test_case::test_case(0, 1; "zero module base")]
#[test_case::test_case(usize::MAX, 1; "address overflow")]
#[should_panic(expected = "Failed to bind required native value Invalid")]
fn invalid_required_binding_panics(base: usize, offset: u64) {
    unsafe { ValueBinding::new("Invalid").initialize(base, offset); }
}

#[test]
#[should_panic(expected = "Required native value Unknown has no binding declaration")]
fn undeclared_value_panics_in_standalone_initialization() {
    unsafe { initialize_value_binding("Unknown", 1); }
}

#[test]
#[should_panic(expected = "already initialized at another address")]
fn required_binding_rejects_rebinding() {
    let binding = ValueBinding::new("Fixed");
    unsafe {
        binding.initialize((&raw const FIRST) as usize, 0);
        binding.initialize((&raw const FIRST) as usize, 0);
        binding.initialize((&raw const SECOND) as usize, 0);
    }
}
