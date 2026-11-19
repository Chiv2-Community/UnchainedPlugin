#![feature(arbitrary_self_types_pointers)]

use unchained_macros::{runtime_bindings, NativeFunction};

struct Add;
struct Borrow;
struct Storage;
struct Missing;

#[repr(C)]
struct Native {
    value: i32,
}

#[runtime_bindings]
impl Native {
    #[bind_resolved_func(Add)]
    unsafe fn add(value: i32) -> i32 {}

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

unsafe extern "C" fn add(value: i32) -> i32 {
    value + 1
}
unsafe extern "C" fn original_trampoline(value: i32) -> i32 {
    value + 2
}
unsafe extern "C" fn borrow(value: &Native) -> &i32 {
    &value.value
}
unsafe extern "C" fn storage<'a>(value: *const Native, output: &'a mut i32) -> &'a mut i32 {
    *output = unsafe { (*value).value };
    output
}

#[test]
fn metadata_preserves_signatures_and_binding_identity() {
    let _: NativeFunction<unsafe extern "C" fn(i32) -> i32> = Native::__UNCHAINED_BINDING_add;
    let _: NativeFunction<for<'a> unsafe extern "C" fn(&'a Native) -> &'a i32> =
        Native::__UNCHAINED_BINDING_borrow;
    let _: NativeFunction<for<'a> unsafe extern "C" fn(*const Native, &'a mut i32) -> &'a mut i32> =
        Native::__UNCHAINED_BINDING_storage;
    let binding = Native::__UNCHAINED_BINDING_add.binding();
    let registration = inventory::iter::<unchained_macros::FunctionBindingRegistration>
        .into_iter()
        .find(|r| r.binding.name() == "Add")
        .unwrap();
    assert!(std::ptr::eq(binding, registration.binding));
    assert!(
        !inventory::iter::<unchained_macros::FunctionBindingRegistration>
            .into_iter()
            .any(|r| r.binding.name() == "DoesNotExist")
    );
}

#[test]
fn wrappers_use_the_entry_then_the_published_trampoline() {
    let binding = Native::__UNCHAINED_BINDING_add.binding();
    let entry = add as *const () as usize;
    unsafe {
        binding.initialize(1, (entry - 1) as u64).unwrap();
        assert_eq!(Native::add(10), 11);
        binding
            .publish_trampoline(original_trampoline as *const () as usize)
            .unwrap();
        assert_eq!(Native::add(10), 12);
        binding.initialize(1, (entry - 1) as u64).unwrap();
        assert_eq!(Native::add(10), 12);
        assert!(binding.publish_trampoline(entry).is_err());
    }
    assert_eq!(binding.entry_address().unwrap().get(), entry);
    assert_ne!(binding.entry_address(), binding.address());
}

#[test]
fn borrowed_receiver_and_return_storage_preserve_ownership() {
    unsafe {
        Native::__UNCHAINED_BINDING_borrow
            .binding()
            .initialize(1, (borrow as *const () as usize - 1) as u64)
            .unwrap();
        Native::__UNCHAINED_BINDING_storage
            .binding()
            .initialize(1, (storage as *const () as usize - 1) as u64)
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
fn uninitialized_calls_and_invalid_trampolines_fail_before_native_execution() {
    assert!(std::panic::catch_unwind(|| unsafe { Native::missing() }).is_err());
    let binding = Native::__UNCHAINED_BINDING_missing.binding();
    assert!(unsafe { binding.publish_trampoline(add as *const () as usize) }.is_err());
    let initialized = unchained_macros::FunctionBinding::new("local");
    unsafe {
        initialized.initialize(1, 1).unwrap();
    }
    assert!(unsafe { initialized.publish_trampoline(0) }.is_err());
}

#[test]
fn duplicate_native_addresses_are_rejected_and_failed_claims_roll_back() {
    let claim = unchained_macros::__private::claim_detour_address(0x12345).unwrap();
    assert!(unchained_macros::__private::claim_detour_address(0x12345).is_err());
    drop(claim);
    unchained_macros::__private::claim_detour_address(0x12345)
        .unwrap()
        .commit();
    assert!(unchained_macros::__private::claim_detour_address(0x12345).is_err());
    assert!(unchained_macros::__private::claim_detour_address(0).is_err());
}
