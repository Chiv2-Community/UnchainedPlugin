#![feature(arbitrary_self_types_pointers)]

use unchained_macros_core::bind_resolved_func;

mod resolvers {
    unchained_macros_patternsleuth::define_pattern_resolver!(Read, ["AA"]);
    unchained_macros_patternsleuth::define_pattern_resolver!(Write, ["AA"]);
    unchained_macros_patternsleuth::define_pattern_resolver!(RawRead, ["AA"]);
    unchained_macros_patternsleuth::define_pattern_resolver!(RawWrite, ["AA"]);
    unchained_macros_patternsleuth::define_pattern_resolver!(Add, ["AA"]);
    unchained_macros_patternsleuth::define_pattern_resolver!(Missing, ["AA"]);
    unchained_macros_patternsleuth::define_pattern_resolver!(Borrow, ["AA"]);
    unchained_macros_patternsleuth::define_pattern_resolver!(ReturnStorage, ["AA"]);
}

#[repr(C)]
struct Native { value: i32 }

impl Native {
    #[bind_resolved_func(resolvers::Read)]
    unsafe fn read(&self, extra: i32) -> i32 {}

    #[bind_resolved_func(resolvers::Write)]
    unsafe fn write(self: &mut Self, value: i32) {}

    #[bind_resolved_func(resolvers::RawRead)]
    unsafe fn raw_read(self: *const Self, extra: i32) -> i32 {}

    #[bind_resolved_func(resolvers::RawWrite)]
    unsafe fn raw_write(self: *mut Self, value: i32) {}

    #[bind_resolved_func(resolvers::Add)]
    unsafe fn add(left: i32, right: i32) -> i32 {}

    #[bind_resolved_func(resolvers::Missing)]
    unsafe fn missing(&self) {}

    #[bind_resolved_func(resolvers::Borrow)]
    unsafe fn borrow<'a>(&'a self) -> &'a i32 {}

    #[bind_resolved_func(resolvers::ReturnStorage)]
    unsafe fn return_storage<'a>(self: *const Self, output: &'a mut i32) -> &'a mut i32 {}
}

unsafe extern "C" fn read(value: *const Native, extra: i32) -> i32 {
    unsafe { (*value).value + extra }
}

unsafe extern "C" fn write(value: *mut Native, replacement: i32) {
    unsafe { (*value).value = replacement; }
}

unsafe extern "C" fn add(left: i32, right: i32) -> i32 { left + right }

unsafe extern "C" fn borrow(value: &Native) -> &i32 { &value.value }

unsafe extern "C" fn return_storage<'a>(value: *const Native, output: &'a mut i32) -> &'a mut i32 {
    *output = unsafe { (*value).value };
    output
}

fn initialize() {
    for registration in unchained_macros_core::binding_inventory_for::<unchained_macros_patternsleuth::PatternsleuthResolver>() {
        let address = match registration.binding.resolver_name() {
            "Read" | "RawRead" => read as *const () as usize,
            "Write" | "RawWrite" => write as *const () as usize,
            "Add" => add as *const () as usize,
            "Missing" => continue,
            "Borrow" => borrow as *const () as usize,
            "ReturnStorage" => return_storage as *const () as usize,
            name => panic!("Unexpected binding {name}"),
        };
        unsafe { registration.binding.initialize_address(std::num::NonZeroUsize::new(address).unwrap()).unwrap(); }
    }
}

#[test_case::test_case(-100; "negative")]
#[test_case::test_case(0; "zero")]
#[test_case::test_case(100; "positive")]
fn generated_bindings_forward_receivers_arguments_and_returns(value: i32) {
    initialize();
    let mut native = Native { value };
    unsafe {
        assert_eq!(native.read(7), value + 7);
        native.write(value + 1);
        assert_eq!(native.value, value + 1);
        assert_eq!((&raw const native).raw_read(3), value + 4);
        (&raw mut native).raw_write(value + 2);
        assert_eq!(native.value, value + 2);
        assert_eq!(Native::add(value, 5), value + 5);
    }
}

#[test]
#[should_panic(expected = "Native binding Missing is not initialized")]
fn uninitialized_binding_fails_before_native_call() {
    unsafe { Native { value: 0 }.missing(); }
}

#[test]
fn binding_initialization_is_idempotent_but_not_rebindable() {
    let binding = unchained_macros_core::FunctionBinding::new("example", <resolvers::Read as unchained_macros_core::ResolverDeclaration>::DESCRIPTOR);
    assert!(binding.address().is_none());
    // This local, unregistered binding is never invoked as native code.
    unsafe { binding.initialize_address(std::num::NonZeroUsize::new(30).unwrap()).unwrap(); }
    unsafe { binding.initialize_address(std::num::NonZeroUsize::new(30).unwrap()).unwrap(); }
    assert_eq!(binding.address().unwrap().get(), 30);
    assert!(unsafe { binding.initialize_address(std::num::NonZeroUsize::new(31).unwrap()) }.is_err());
    assert_eq!(binding.address().unwrap().get(), 30);
}

#[test]
fn generated_bindings_preserve_receiver_and_return_storage_lifetimes() {
    initialize();
    let native = Native { value: 42 };
    let mut output = 0;
    let address = &raw mut output;
    unsafe {
        assert!(std::ptr::eq(native.borrow(), &native.value));
        let result = (&raw const native).return_storage(&mut output);
        assert_eq!(*result, 42);
        assert_eq!(result as *mut i32, address);
    }
}

#[test_case::test_case(0, 1; "missing module")]
#[test_case::test_case(usize::MAX, 1; "address overflow")]
fn invalid_initialization_leaves_binding_uninitialized(base: usize, offset: u64) {
    let binding = unchained_macros_core::FunctionBinding::new("invalid", <resolvers::Read as unchained_macros_core::ResolverDeclaration>::DESCRIPTOR);
    use unchained_macros_core::AddressResolver;
    let context = unchained_macros_patternsleuth::ScanContext::new(base,
        &std::collections::HashMap::from([("Read".into(), offset)]));
    assert!(unsafe { binding.resolver.implementation.resolve(&context) }.is_err());
    assert!(binding.address().is_none());
}

#[test]
fn one_time_address_publication_supports_cross_thread_readers() {
    static VALUE: usize = 42;
    let address = (&raw const VALUE) as usize;
    let binding = unchained_macros_core::ResolvedAddress::new("SharedFixture");
    assert!(binding.get().is_none());
    unsafe { binding.try_initialize(1, (address - 1) as u64).unwrap(); }
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| assert_eq!(binding.get().unwrap().get(), address));
        }
    });
}
