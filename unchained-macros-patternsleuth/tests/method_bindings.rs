#![feature(arbitrary_self_types_pointers)]

use unchained_macros_core::{runtime_bindings, NativeTarget};

unchained_macros_patternsleuth::define_pattern_resolver!(Add, ["AA"]);

unchained_macros_patternsleuth::define_pattern_resolver!(Borrow, ["AA"]);

unchained_macros_patternsleuth::define_pattern_resolver!(Storage, ["AA"]);

unchained_macros_patternsleuth::define_pattern_resolver!(Missing, ["AA"]);

#[repr(C)]

struct Native {
    value: i32,
}

#[runtime_bindings]

impl Native {
    #[bind_resolved_func(Add)]

    unsafe fn add(native: i32) -> i32 {}

    #[bind_resolved_func(Borrow)]

    unsafe fn borrow(&self) -> &i32 {}

    #[bind_resolved_func(Storage)]

    unsafe fn storage<'a>(self: *const Self, output: &'a mut i32) -> &'a mut i32 {}

    #[bind_resolved_func(Missing)]

    unsafe fn missing() {}

    #[cfg(any())]
    #[bind_resolved_func(DoesNotExist)]

    unsafe fn excluded() {}
}

#[inline(never)]

unsafe extern "C" fn add(value: i32) -> i32 {

    unsafe {

        std::ptr::read_volatile(&value)
    }
    .wrapping_add(1)
}

unsafe extern "C" fn original_trampoline(value: i32) -> i32 {

    value + 2
}

unsafe extern "C" fn borrow(value: &Native) -> &i32 {

    &value.value
}

unsafe extern "C" fn storage<'a>(value: *const Native, output: &'a mut i32) -> &'a mut i32 {

    *output = unsafe {

        (*value).value
    };

    output
}

#[test]

fn metadata_preserves_signatures_and_binding_identity() {

    let _: &NativeTarget<
        unsafe extern "C" fn(i32) -> i32,
        _,
        _,
        _,
        unchained_macros_patternsleuth::PatternsleuthResolver,
    > = Native::__UNCHAINED_TARGET_add;

    let _: &NativeTarget<
        for<'a> unsafe extern "C" fn(&'a Native) -> &'a i32,
        _,
        _,
        _,
        unchained_macros_patternsleuth::PatternsleuthResolver,
    > = Native::__UNCHAINED_TARGET_borrow;

    let _: &NativeTarget<
        for<'a> unsafe extern "C" fn(*const Native, &'a mut i32) -> &'a mut i32,
        _,
        _,
        _,
        unchained_macros_patternsleuth::PatternsleuthResolver,
    > = Native::__UNCHAINED_TARGET_storage;

    let binding = Native::__UNCHAINED_TARGET_add.binding();

    let registration =
        unchained_macros_core::binding_inventory_for::<unchained_macros_patternsleuth::PatternsleuthResolver>()
            .find(|r| r.name() == "Native::add")
            .unwrap();

    assert!(std::ptr::eq(binding, registration));

    assert!(
        !unchained_macros_core::binding_inventory_for::<unchained_macros_patternsleuth::PatternsleuthResolver>()
            .any(|r| r.resolver_name() == "DoesNotExist")
    );
}

#[test]

fn wrappers_use_the_entry_then_the_published_trampoline() {

    let binding = Native::__UNCHAINED_TARGET_add.binding();

    let entry = add as *const () as usize;

    unsafe {

        binding
            .initialize_address(std::num::NonZeroUsize::new(entry).unwrap())
            .unwrap();

        assert_eq!(Native::add(10), 11);

        let detour = Box::leak(Box::new(
            retour::RawDetour::new(add as *const (), original_trampoline as *const ()).unwrap(),
        ));

        binding.publish_detour(detour).unwrap();

        detour.enable().unwrap();

        assert_eq!(std::hint::black_box(add as unsafe extern "C" fn(i32) -> i32)(10), 12);

        assert_eq!(Native::add(10), 11);

        binding
            .initialize_address(std::num::NonZeroUsize::new(entry).unwrap())
            .unwrap();

        assert_eq!(Native::add(10), 11);

        assert!(binding.publish_detour(detour).is_err());

        detour.disable().unwrap();
    }

    assert_eq!(binding.entry_address().unwrap().get(), entry);

    assert_ne!(binding.entry_address(), binding.address());
}

#[test]

fn borrowed_receiver_and_return_storage_preserve_ownership() {

    unsafe {

        Native::__UNCHAINED_TARGET_borrow
            .binding()
            .initialize_address(std::num::NonZeroUsize::new(borrow as *const () as usize).unwrap())
            .unwrap();

        Native::__UNCHAINED_TARGET_storage
            .binding()
            .initialize_address(std::num::NonZeroUsize::new(storage as *const () as usize).unwrap())
            .unwrap();

        let native = Native { value: 42 };

        assert!(std::ptr::eq(native.borrow(), &native.value));

        let mut out = 0;

        let address = &raw mut out;

        let result = (&raw const native).storage(&mut out);

        assert_eq!(result as *mut _, address);

        assert_eq!(*result, 42);
    }
}

#[test]

fn uninitialized_calls_and_detour_publication_fail_before_native_execution() {

    assert!(std::panic::catch_unwind(|| unsafe {

        Native::missing()
    })
    .is_err());

    let binding = Native::__UNCHAINED_TARGET_missing.binding();

    #[inline(never)]

    unsafe extern "C" fn unbound_target(value: i32) -> i32 {

        unsafe {

            std::ptr::read_volatile(&value)
        }
        .wrapping_add(3)
    }

    let detour = Box::leak(Box::new(unsafe {

        retour::RawDetour::new(unbound_target as *const (), original_trampoline as *const ()).unwrap()
    }));

    assert!(unsafe {

        binding.publish_detour(detour)
    }
    .is_err());
}

#[test]

fn duplicate_native_addresses_are_rejected_and_failed_claims_roll_back() {

    let claim = unchained_macros_core::__private::claim_detour_address(0x12345).unwrap();

    assert!(unchained_macros_core::__private::claim_detour_address(0x12345).is_err());

    drop(claim);

    unchained_macros_core::__private::claim_detour_address(0x12345)
        .unwrap()
        .commit();

    assert!(unchained_macros_core::__private::claim_detour_address(0x12345).is_err());

    assert!(unchained_macros_core::__private::claim_detour_address(0).is_err());
}
