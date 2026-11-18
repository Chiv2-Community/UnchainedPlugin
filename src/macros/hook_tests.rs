CREATE_HOOK!(TestVoidBasic, (value: i32), {});
CREATE_HOOK!(TestVoidStatus, INACTIVE, (value: i32), {});
CREATE_HOOK!(TestVoidCondition, { || false }, (value: i32), {});
CREATE_HOOK!(TestConstPointer, INACTIVE, *const std::ffi::c_void,
    (value: *const std::ffi::c_void), {});

#[test]
fn shorthand_hooks_return_unit() {
    // Check signatures without attaching or calling uninitialized detours.
    let _: unsafe extern "C" fn(i32) = TestVoidBasic_detour_fkt;
    let _: unsafe extern "C" fn(i32) = TestVoidStatus_detour_fkt;
    let _: unsafe extern "C" fn(i32) = TestVoidCondition_detour_fkt;
    let _: fn(i32) = |value| CALL_ORIGINAL!(TestVoidBasic(value));
    let _: fn(i32) = |value| CALL_ORIGINAL!(TestVoidStatus(value));
    let _: fn(i32) = |value| CALL_ORIGINAL!(TestVoidCondition(value));
}

#[test]
fn explicit_const_pointer_return_is_preserved() {
    type Pointer = *const std::ffi::c_void;
    let _: unsafe extern "C" fn(Pointer) -> Pointer = TestConstPointer_detour_fkt;
    let _: fn(Pointer) -> Pointer = |value| CALL_ORIGINAL!(TestConstPointer(value));
}

CREATE_HOOK!(TestBorrowed, for<'a>, INACTIVE, PRE, &'a i32, (value: &'a i32), {});
CREATE_HOOK!(TestMutBorrowed, for<'a>, INACTIVE, PRE, &'a mut i32,
    (value: &'a mut i32), { *value += 2; });

#[inline(never)]
unsafe extern "C" fn borrowed_target(value: &i32) -> &i32 {
    std::hint::black_box(value)
}

#[inline(never)]
unsafe extern "C" fn mut_borrowed_target(value: &mut i32) -> &mut i32 {
    *value += 3;
    std::hint::black_box(value)
}

#[test]
fn lifetime_hooks_preserve_borrows_and_disabled_original_calls() {
    let _: for<'a> unsafe extern "C" fn(&'a i32) -> &'a i32 = TestBorrowed_detour_fkt;
    let _: for<'a> unsafe extern "C" fn(&'a mut i32) -> &'a mut i32 = TestMutBorrowed_detour_fkt;
    let base = *crate::resolvers::BASE_ADDR;
    let offsets = std::collections::HashMap::from([
        ("TestBorrowed".into(), (borrowed_target as *const () as usize - base) as u64),
        ("TestMutBorrowed".into(), (mut_borrowed_target as *const () as usize - base) as u64),
    ]);
    unsafe {
        attach_TestBorrowed(offsets.clone(), true).unwrap();
        attach_TestMutBorrowed(offsets, true).unwrap();
        assert!(!o_TestBorrowed.get().unwrap().is_enabled());
        assert!(!o_TestMutBorrowed.get().unwrap().is_enabled());
        let input = 17;
        assert!(std::ptr::eq(TestBorrowed_detour_fkt(&input), &input));
        assert!(std::ptr::eq(CALL_ORIGINAL_SAFE!(TestBorrowed(&input)).unwrap(), &input));
        let mut input = 10;
        let address = &raw mut input;
        let result = TestMutBorrowed_detour_fkt(&mut input);
        assert_eq!(*result, 15);
        assert_eq!(result as *mut i32, address);
        let hook = o_TestMutBorrowed.get().unwrap();
        hook.enable().unwrap();
        let target = std::hint::black_box(mut_borrowed_target as unsafe extern "C" fn(&mut i32) -> &mut i32);
        let mut intercepted = 20;
        let value = *target(&mut intercepted);
        hook.disable().unwrap();
        assert_eq!(value, 25);
    }
}
