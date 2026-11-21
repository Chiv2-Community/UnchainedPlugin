#![feature(arbitrary_self_types_pointers)]
use std::sync::atomic::{AtomicUsize, Ordering};
use unchained_macros::{bind_resolved_value, runtime_bindings, initialize_value_bindings};

mod resolvers {
    pub struct Allocator;
    pub struct Inline;
    pub struct Probe;
}

#[runtime_bindings(global)]
struct TestAllocator {
    calls: AtomicUsize,
}

#[runtime_bindings(global)]
impl TestAllocator {
    pub unsafe fn allocate(self: *mut Self, amount: usize) -> *mut Self {
        unsafe { (*self).calls.fetch_add(amount, Ordering::Relaxed); }
        self
    }

    pub unsafe fn identity(self: *const Self) -> *const Self { self }

    #[unchained_macros::bind_resolved_func(resolvers::Probe)]
    pub unsafe fn probe(self: *const Self) -> usize {}

    #[cfg(any())]
    pub unsafe fn unavailable(&self, missing: MissingType) {}
}

#[runtime_bindings(global)]
struct InlineObject(AtomicUsize);

#[runtime_bindings(global)]
impl InlineObject {
    pub fn borrow<'a>(&'a self) -> &'a AtomicUsize { &self.0 }
    pub fn address(&self) -> *const Self { self }
}

#[bind_resolved_value(resolvers::Allocator, Value)]
static Allocator: *mut TestAllocator;
#[bind_resolved_value(resolvers::Inline, Address)]
static Inline: *mut InlineObject;

// A disabled declaration must not emit registrations or require its target.
#[cfg(any())]
#[bind_resolved_value(resolvers::Missing, Address)]
static Disabled: *mut MissingType;

#[test]
fn static_globals_reload_pointer_slots_and_preserve_inline_storage() {
    assert!(std::panic::catch_unwind(|| unsafe { Allocator.allocate(1) }).is_err());
    assert!(std::panic::catch_unwind(|| unsafe { Inline.address() }).is_err());
    let mut slot: *mut TestAllocator = std::ptr::null_mut();
    let object = InlineObject(AtomicUsize::new(0));
    let offsets = std::collections::HashMap::from([
        ("Allocator".into(), ((&raw mut slot) as usize - 1) as u64),
        ("Inline".into(), ((&raw const object) as usize - 1) as u64),
    ]);
    // The registry does not dereference the slot during binding.
    unsafe { initialize_value_bindings(1, &offsets); }
    unsafe extern "C" fn probe(allocator: *const TestAllocator) -> usize {
        unsafe { (*allocator).calls.load(Ordering::Relaxed) }
    }
    unsafe {
        unchained_macros::initialize_function_bindings(1, &std::collections::HashMap::from([
            ("Probe".into(), (probe as *const () as usize - 1) as u64),
        ])).unwrap();
    }
    assert!(std::panic::catch_unwind(|| unsafe { Allocator.allocate(1) }).is_err());
    let mut first = TestAllocator { calls: AtomicUsize::new(0) };
    let mut second = TestAllocator { calls: AtomicUsize::new(0) };
    // Native-style ordinary writes and reads are sequential here, never racing.
    slot = &raw mut first;
    assert_eq!(slot, &raw mut first);
    assert_eq!(unsafe { Allocator.allocate(3) }, &raw mut first);
    assert_eq!(unsafe { Allocator.identity() }, &raw const first);
    assert_eq!(unsafe { Allocator.probe() }, 3);
    slot = &raw mut second;
    assert_eq!(slot, &raw mut second);
    assert_eq!(unsafe { Allocator.allocate(7) }, &raw mut second);
    assert_eq!(unsafe { Allocator.probe() }, 7);
    assert_eq!(first.calls.load(Ordering::Relaxed), 3);
    assert_eq!(second.calls.load(Ordering::Relaxed), 7);
    assert_eq!(unsafe { Inline.address() }, &raw const object);
    let borrowed = unsafe { Inline.borrow() };
    borrowed.store(19, Ordering::Relaxed);
    assert!(std::ptr::eq(borrowed, &object.0));
    assert_eq!(object.0.load(Ordering::Relaxed), 19);
}
