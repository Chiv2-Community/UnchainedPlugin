#![cfg(windows)]

struct ConstRead;
struct Native(i32);

#[unchained_macros::runtime_bindings]
impl Native {
    #[bind_resolved_func(ConstRead)]
    unsafe fn read(value: *const Self) -> i32 {}
}

#[unchained_macros::bind_func_detour(Native::read)]
unsafe fn replacement(#[native_const] value: *mut Native) -> i32 {
    // The native caller lends a const object; adaptation does not authorize writes.
    unsafe { Native::read(value) + 10 }
}

#[inline(never)]
unsafe extern "C" fn original(value: *const Native) -> i32 {
    unsafe { (*std::hint::black_box(value)).0 + 1 }
}

#[test]
fn native_const_preserves_original_calls_and_const_native_entry() {
    unsafe {
        Native::__UNCHAINED_BINDING_read
            .binding()
            .initialize(1, original as *const () as u64 - 1)
            .unwrap();
        let value = Native(42);
        let pointer = &raw const value;
        let call = std::hint::black_box(original as unsafe extern "C" fn(*const Native) -> i32);
        assert_eq!(call(pointer), 43);
        __unchained_attach_replacement(1, true).unwrap();
        assert_eq!(call(pointer), 53);
        assert_eq!(Native::read(pointer), 43);
        assert_eq!(value.0, 42);
        __UNCHAINED_HOOK_replacement
            .get()
            .unwrap()
            .disable()
            .unwrap();
        assert_eq!(call(pointer), 43);
    }
}
