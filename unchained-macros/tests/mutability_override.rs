#![cfg(windows)]

struct ConstRead;
struct MutRead;
struct ConstMatch;
struct MutMatch;
struct Borrowed;
struct SharedRead;
struct ExclusiveRead;
struct Native(i32);

#[unchained_macros::runtime_bindings]
impl Native {
    #[bind_resolved_func(ConstRead)]
    unsafe fn read(value: *const Self) -> i32 {}

    #[bind_resolved_func(MutRead)]
    unsafe fn mut_read(value: *mut Self, other: *const i32) -> i32 {}

    #[bind_resolved_func(ConstMatch)]
    unsafe fn const_match(value: *const Self) -> i32 {}

    #[bind_resolved_func(MutMatch)]
    unsafe fn mut_match<'a>(value: *mut Self, other: &'a i32) -> &'a i32 {}

    #[bind_resolved_func(Borrowed)]
    unsafe fn borrowed<'a>(value: *const &'a i32) -> &'a i32 {}

    #[bind_resolved_func(SharedRead)]
    unsafe fn shared_read(value: &Self) -> &i32 {}

    #[bind_resolved_func(ExclusiveRead)]
    unsafe fn exclusive_read(value: &mut Self) -> &i32 {}
}

#[unchained_macros::bind_func_detour(Native::mut_read)]
unsafe fn reverse(#[mutability_override] value: *const Native, other: *const i32) -> i32 {
    // Reading through a const view does not alter the mutable native contract.
    unsafe { Native::mut_read(value.cast_mut(), other) + 20 }
}

#[unchained_macros::bind_func_detour(Native::const_match)]
unsafe fn const_match(#[mutability_override] value: *const Native) -> i32 {
    unsafe { Native::const_match(value) + 30 }
}

#[unchained_macros::bind_func_detour(Native::mut_match)]
unsafe fn mut_match<'a>(#[mutability_override] value: *mut Native, other: &'a i32) -> &'a i32 {
    unsafe { Native::mut_match(value, other) }
}

#[unchained_macros::bind_func_detour(Native::borrowed)]
unsafe fn borrowed<'a>(#[mutability_override] value: *mut &'a i32) -> &'a i32 {
    unsafe { Native::borrowed(value) }
}

#[unchained_macros::bind_func_detour(Native::shared_read)]
unsafe fn shared_read<'a>(#[mutability_override] value: &'a mut Native) -> &'a i32 {
    // This fixture's raw native caller provides exclusive, writable storage.
    value.0 += 1;
    unsafe { Native::shared_read(value) }
}

#[unchained_macros::bind_func_detour(Native::exclusive_read)]
unsafe fn exclusive_read<'a>(#[mutability_override] value: &'a Native) -> &'a i32 {
    &value.0
}

#[inline(never)]
unsafe extern "C" fn original_borrowed<'a>(value: *const &'a i32) -> &'a i32 {
    unsafe { *std::hint::black_box(value) }
}

#[inline(never)]
unsafe extern "C" fn original_shared(value: &Native) -> &i32 {
    &std::hint::black_box(value).0
}

#[inline(never)]
unsafe extern "C" fn original_exclusive(value: &mut Native) -> &i32 {
    // Keep this target semantically distinct from original_shared under ICF.
    let value = std::hint::black_box(value);
    value.0 += 2;
    &value.0
}

#[test]
fn reference_overrides_and_nested_borrows_preserve_lifetimes() {
    assert_ne!(original_shared as *const (), original_exclusive as *const ());
    unsafe {
        Native::__UNCHAINED_BINDING_borrowed.binding()
            .initialize(1, original_borrowed as *const () as u64 - 1).unwrap();
        Native::__UNCHAINED_BINDING_shared_read.binding()
            .initialize(1, original_shared as *const () as u64 - 1).unwrap();
        Native::__UNCHAINED_BINDING_exclusive_read.binding()
            .initialize(1, original_exclusive as *const () as u64 - 1).unwrap();
        let number = 17;
        let borrowed_number = &number;
        let borrowed_call = std::hint::black_box(original_borrowed as for<'a> unsafe extern "C" fn(*const &'a i32) -> &'a i32);
        __unchained_attach_borrowed(1, true).unwrap();
        assert!(std::ptr::eq(borrowed_call(&borrowed_number), &number));
        assert!(std::ptr::eq(Native::borrowed(&borrowed_number), &number));

        // Model native C++ calls through the equivalent raw-pointer ABI. No
        // shared Rust borrow is live when the replacement receives &mut Native.
        let shared_call: unsafe extern "C" fn(*mut Native) -> *const i32 =
            std::mem::transmute(std::hint::black_box(original_shared as *const ()));
        let exclusive_call: unsafe extern "C" fn(*mut Native) -> *const i32 =
            std::mem::transmute(std::hint::black_box(original_exclusive as *const ()));
        let mut value = Native(42);
        let pointer = &raw mut value;
        __unchained_attach_shared_read(1, true).unwrap();
        __unchained_attach_exclusive_read(1, true).unwrap();
        assert_eq!(*shared_call(pointer), 43);
        assert_eq!(*exclusive_call(pointer), 43);
        assert_eq!(*Native::shared_read(&value), 43);
        assert_eq!(*Native::exclusive_read(&mut value), 45);
        __UNCHAINED_HOOK_borrowed.get().unwrap().disable().unwrap();
        __UNCHAINED_HOOK_shared_read.get().unwrap().disable().unwrap();
        __UNCHAINED_HOOK_exclusive_read.get().unwrap().disable().unwrap();
        let pointer = &raw mut value;
        assert_eq!(*shared_call(pointer), 45);
        assert_eq!(*exclusive_call(pointer), 47);
    }
}

#[inline(never)]
unsafe extern "C" fn original_mut(value: *mut Native, other: *const i32) -> i32 {
    unsafe { (*std::hint::black_box(value)).0 + *other }
}

#[inline(never)]
unsafe extern "C" fn original_const_match(value: *const Native) -> i32 {
    unsafe { (*std::hint::black_box(value)).0 + 2 }
}

#[inline(never)]
unsafe extern "C" fn original_mut_match<'a>(value: *mut Native, other: &'a i32) -> &'a i32 {
    std::hint::black_box(value);
    std::hint::black_box(other)
}

#[test]
fn reverse_and_exact_matches_keep_native_signatures_and_original_calls() {
    unsafe {
        Native::__UNCHAINED_BINDING_mut_read.binding()
            .initialize(1, original_mut as *const () as u64 - 1).unwrap();
        Native::__UNCHAINED_BINDING_const_match.binding()
            .initialize(1, original_const_match as *const () as u64 - 1).unwrap();
        Native::__UNCHAINED_BINDING_mut_match.binding()
            .initialize(1, original_mut_match as *const () as u64 - 1).unwrap();
        let mut value = Native(42);
        let pointer = &raw mut value;
        let other = 7;
        let reverse_call = std::hint::black_box(original_mut as unsafe extern "C" fn(*mut Native, *const i32) -> i32);
        let const_call = std::hint::black_box(original_const_match as unsafe extern "C" fn(*const Native) -> i32);
        let mut_call = std::hint::black_box(original_mut_match as for<'a> unsafe extern "C" fn(*mut Native, &'a i32) -> &'a i32);
        __unchained_attach_reverse(1, true).unwrap();
        __unchained_attach_const_match(1, true).unwrap();
        __unchained_attach_mut_match(1, true).unwrap();
        assert_eq!(reverse_call(pointer, &other), 69);
        assert_eq!(Native::mut_read(pointer, &other), 49);
        assert_eq!(const_call(pointer), 74);
        assert_eq!(Native::const_match(pointer), 44);
        assert!(std::ptr::eq(mut_call(pointer, &other), &other));
        assert!(std::ptr::eq(Native::mut_match(pointer, &other), &other));
        assert_eq!(value.0, 42);
        __UNCHAINED_HOOK_reverse.get().unwrap().disable().unwrap();
        __UNCHAINED_HOOK_const_match.get().unwrap().disable().unwrap();
        __UNCHAINED_HOOK_mut_match.get().unwrap().disable().unwrap();
        assert_eq!(reverse_call(pointer, &other), 49);
        assert_eq!(const_call(pointer), 44);
        assert!(std::ptr::eq(mut_call(pointer, &other), &other));
    }
}

#[unchained_macros::bind_func_detour(Native::read)]
unsafe fn replacement(#[mutability_override] value: *mut Native) -> i32 {
    // The native caller lends a const object; adaptation does not authorize writes.
    unsafe { Native::read(value) + 10 }
}

#[inline(never)]
unsafe extern "C" fn original(value: *const Native) -> i32 {
    unsafe { (*std::hint::black_box(value)).0 + 1 }
}

#[test]
fn mutability_override_preserves_original_calls_and_const_native_entry() {
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
