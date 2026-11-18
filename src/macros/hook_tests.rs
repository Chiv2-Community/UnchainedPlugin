CREATE_HOOK!(TestVoidBasic, (value: i32), {});

mod resolved_detours {
    use std::sync::atomic::{AtomicBool, Ordering};
    use unchained_macros::bind_resolved_func_detour;

    struct AttributeAdd;
    struct AttributeBorrow;
    struct AttributeZero;
    struct AttributeGlobalOff;
    static ENABLED: AtomicBool = AtomicBool::new(false);

    #[bind_resolved_func_detour(self::AttributeAdd, || ENABLED.load(Ordering::SeqCst))]
    fn add_detour(value: i32) -> i32 { unsafe { add(value) + 10 } }

    #[bind_resolved_func_detour(AttributeBorrow, INACTIVE)]
    unsafe fn borrow_detour<'a>(value: &'a mut i32) -> &'a mut i32 {
        *value += 2;
        unsafe { borrow(value) }
    }

    #[bind_resolved_func_detour(AttributeZero)]
    fn zero_detour() -> i32 { unsafe { zero() + 1 } }

    #[bind_resolved_func_detour(AttributeGlobalOff, ACTIVE)]
    fn global_off_detour(value: i32) -> i32 { unsafe { global_off(value) + 100 } }

    #[cfg(any())]
    #[bind_resolved_func_detour(DoesNotExist)]
    fn excluded_detour() {}

    #[inline(never)]
    unsafe extern "C" fn add_target(value: i32) -> i32 { std::hint::black_box(value) + 3 }
    #[inline(never)]
    unsafe extern "C" fn borrow_target(value: &mut i32) -> &mut i32 {
        *value += 3;
        std::hint::black_box(value)
    }
    #[inline(never)]
    unsafe extern "C" fn zero_target() -> i32 { std::hint::black_box(17) }
    #[inline(never)]
    unsafe extern "C" fn off_target(value: i32) -> i32 { std::hint::black_box(value) + 4 }

    #[test]
    fn registered_attributes_attach_and_call_trampolines() {
        let base = *crate::resolvers::BASE_ADDR;
        let offsets = std::collections::HashMap::from([
            ("AttributeAdd".into(), (add_target as *const () as usize - base) as u64),
            ("AttributeBorrow".into(), (borrow_target as *const () as usize - base) as u64),
            ("AttributeZero".into(), (zero_target as *const () as usize - base) as u64),
            ("AttributeGlobalOff".into(), (off_target as *const () as usize - base) as u64),
        ]);
        let registration = |name| inventory::iter::<crate::resolvers::HookRegistration>
            .into_iter().find(|r| r.name == name).unwrap();
        let add_registration = registration("AttributeAdd");
        assert!(!(add_registration.condition)());
        ENABLED.store(true, Ordering::SeqCst);
        assert!((add_registration.condition)());
        unsafe {
            assert!(!unchained_macros::detour_is_initialized("AttributeAdd"));
            assert!(!unchained_macros::detour_is_enabled("AttributeAdd"));
            assert!(!unchained_macros::detour_is_initialized("MissingDetour"));
            assert!(std::panic::catch_unwind(|| add(5)).is_err());
            assert!((add_registration.hook_fn)(Default::default(), true).is_err());
            assert!((add_registration.hook_fn)(
                std::collections::HashMap::from([("AttributeAdd".into(), u64::MAX)]), true,
            ).is_err());
            (add_registration.hook_fn)(offsets.clone(), true).unwrap();
            (registration("AttributeBorrow").hook_fn)(offsets.clone(), true).unwrap();
            (registration("AttributeZero").hook_fn)(offsets.clone(), true).unwrap();
            (registration("AttributeGlobalOff").hook_fn)(offsets.clone(), false).unwrap();
            assert!(unchained_macros::detour_is_initialized("AttributeAdd"));
            assert!(unchained_macros::detour_is_enabled("AttributeAdd"));
            assert!(unchained_macros::detour_is_initialized("AttributeGlobalOff"));
            assert!(!unchained_macros::detour_is_enabled("AttributeGlobalOff"));
            let add_call = std::hint::black_box(add_target as unsafe extern "C" fn(i32) -> i32);
            let zero_call = std::hint::black_box(zero_target as unsafe extern "C" fn() -> i32);
            assert_eq!(add_call(5), 18);
            assert_eq!(add(5), 8);
            assert!((add_registration.hook_fn)(offsets, true).is_err());
            assert_eq!(zero_call(), 18);
            assert_eq!(zero(), 17);
            assert!(!__UNCHAINED_HOOK_borrow_detour.get().unwrap().is_enabled());
            assert!(!__UNCHAINED_HOOK_global_off_detour.get().unwrap().is_enabled());
            assert_eq!(global_off(5), 9);
            let mut input = 10;
            let ptr = &raw mut input;
            let result = borrow_detour(&mut input);
            assert_eq!(*result, 15);
            assert_eq!(result as *mut i32, ptr);
            __UNCHAINED_HOOK_borrow_detour.get().unwrap().enable().unwrap();
            let borrow_call = std::hint::black_box(
                borrow_target as unsafe extern "C" fn(&mut i32) -> &mut i32);
            assert_eq!(*borrow_call(&mut input), 20);
            __UNCHAINED_HOOK_borrow_detour.get().unwrap().disable().unwrap();
            __UNCHAINED_HOOK_add_detour.get().unwrap().disable().unwrap();
            __UNCHAINED_HOOK_zero_detour.get().unwrap().disable().unwrap();
        }
    }
}
CREATE_HOOK!(TestVoidStatus, INACTIVE, (value: i32), {});

mod runtime_binding_methods {
    use unchained_macros::runtime_bindings;

    struct MethodAdd;
    struct MethodBorrow;
    struct MethodRaw;
    struct MethodMutBorrow;
    struct MethodArgBorrow;
    struct MethodMake;
    struct OtherAdd;

    #[repr(C)]
    struct Native { value: i32 }

    #[runtime_bindings]
    impl Native {
        fn enabled() -> bool { true }

        #[bind_resolved_func_detour(MethodAdd, || Self::enabled())]
        fn foo_detour(&mut self, value: i32) -> i32 {
            unsafe { self.foo(value) + 1 }
        }

        #[bind_resolved_func_detour(MethodBorrow, INACTIVE)]
        fn borrow_detour(&self) -> &Self { unsafe { self.borrow() } }

        #[unchained_macros::bind_resolved_func_detour(MethodRaw, DISABLED)]
        unsafe fn raw_detour(self: *mut Self, extra: i32) -> i32 {
            unsafe { self.raw(extra) + 2 }
        }

        #[bind_resolved_func_detour(MethodMutBorrow, INACTIVE)]
        fn mut_borrow_detour(&mut self) -> &mut i32 { unsafe { self.mut_borrow() } }

        #[bind_resolved_func_detour(MethodArgBorrow, DISABLED)]
        unsafe fn arg_borrow_detour<'a>(&self, value: &'a i32) -> &'a i32 {
            unsafe { self.arg_borrow(value) }
        }

        #[bind_resolved_func_detour(MethodMake, INACTIVE)]
        fn make_detour(value: i32) -> Self { unsafe { Self::make(value) } }

        #[cfg(any())]
        #[bind_resolved_func_detour(DoesNotExist)]
        fn absent_detour(&mut self) {}
    }

    struct Other;

    #[runtime_bindings]
    impl Other {
        #[bind_resolved_func_detour(OtherAdd)]
        fn foo_detour(&mut self, value: i32) -> i32 {
            unsafe { self.foo(value) + 10 }
        }
    }

    #[inline(never)]
    unsafe extern "C" fn add_target(this: &mut Native, value: i32) -> i32 {
        this.value += std::hint::black_box(value);
        this.value
    }

    #[inline(never)]
    unsafe extern "C" fn borrow_target(this: &Native) -> &Native { std::hint::black_box(this) }

    #[inline(never)]
    unsafe extern "C" fn raw_target(this: *mut Native, extra: i32) -> i32 {
        unsafe { (*std::hint::black_box(this)).value + extra }
    }

    #[inline(never)]
    unsafe extern "C" fn other_target(_this: &mut Other, value: i32) -> i32 {
        std::hint::black_box(value) * 2
    }

    #[inline(never)]
    unsafe extern "C" fn mut_borrow_target(this: &mut Native) -> &mut i32 {
        std::hint::black_box(&mut this.value)
    }

    #[inline(never)]
    unsafe extern "C" fn arg_borrow_target<'a>(_this: &Native, value: &'a i32) -> &'a i32 {
        std::hint::black_box(value)
    }

    #[inline(never)]
    unsafe extern "C" fn make_target(value: i32) -> Native {
        Native { value: std::hint::black_box(value) }
    }

    #[test]
    fn impl_detours_generate_suffix_stripped_trampoline_methods() {
        let base = *crate::resolvers::BASE_ADDR;
        let offsets = std::collections::HashMap::from([
            ("MethodAdd".into(), (add_target as *const () as usize - base) as u64),
            ("MethodBorrow".into(), (borrow_target as *const () as usize - base) as u64),
            ("MethodRaw".into(), (raw_target as *const () as usize - base) as u64),
            ("OtherAdd".into(), (other_target as *const () as usize - base) as u64),
            ("MethodMutBorrow".into(), (mut_borrow_target as *const () as usize - base) as u64),
            ("MethodArgBorrow".into(), (arg_borrow_target as *const () as usize - base) as u64),
            ("MethodMake".into(), (make_target as *const () as usize - base) as u64),
        ]);
        for name in ["MethodAdd", "MethodBorrow", "MethodRaw", "OtherAdd",
            "MethodMutBorrow", "MethodArgBorrow", "MethodMake"] {
            let registration = inventory::iter::<crate::resolvers::HookRegistration>
                .into_iter().find(|r| r.name == name).unwrap();
            unsafe { (registration.hook_fn)(offsets.clone(), true).unwrap(); }
        }
        let mut native = Native { value: 10 };
        unsafe {
            let call = std::hint::black_box(add_target as unsafe extern "C" fn(&mut Native, i32) -> i32);
            assert_eq!(call(&mut native, 5), 16);
            assert_eq!(native.value, 15);
            assert_eq!(native.foo(2), 17);
            assert_eq!(native.foo_detour(2), 20);
            assert!(std::ptr::eq(native.borrow(), &native));
            assert!(std::ptr::eq(native.borrow_detour(), &native));
            let raw = &raw mut native;
            assert_eq!(raw.raw(3), 22);
            assert_eq!(raw.raw_detour(3), 24);
            let field = &raw mut native.value;
            assert_eq!(native.mut_borrow() as *mut i32, field);
            assert_eq!(native.mut_borrow_detour() as *mut i32, field);
            let value = 42;
            assert!(std::ptr::eq(native.arg_borrow(&value), &value));
            assert!(std::ptr::eq(native.arg_borrow_detour(&value), &value));
            assert_eq!(Native::make(7).value, 7);
            assert_eq!(Native::make_detour(9).value, 9);
            let other = std::hint::black_box(other_target as unsafe extern "C" fn(&mut Other, i32) -> i32);
            assert_eq!(other(&mut Other, 4), 18);
            assert_eq!(Other.foo(4), 8);
        }
    }
}
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
