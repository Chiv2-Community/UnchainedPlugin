#![cfg(windows)]
#![feature(arbitrary_self_types_pointers)]
#![allow(non_snake_case)]

mod model {

    unchained_macros_patternsleuth::define_pattern_resolver!(DetachedAdd, ["AA"]);

    unchained_macros_patternsleuth::define_pattern_resolver!(DetachedAlias, ["AA"]);

    unchained_macros_patternsleuth::define_pattern_resolver!(DetachedBorrow, ["AA"]);

    unchained_macros_patternsleuth::define_pattern_resolver!(DetachedStorage, ["AA"]);

    unchained_macros_patternsleuth::define_pattern_resolver!(DetachedGlobalOff, ["AA"]);

    unchained_macros_patternsleuth::define_pattern_resolver!(DetachedZero, ["AA"]);

    unchained_macros_patternsleuth::define_offset_resolver!(GetPost_Detached);

    #[repr(C)]
    pub struct Native {
        pub value: i32,
    }

    #[unchained_macros_core::runtime_bindings]

    impl Native {
        #[bind_resolved_func(DetachedAdd)]

        pub unsafe fn add(value: i32) -> i32 {}

        #[bind_resolved_func(DetachedAlias)]

        pub unsafe fn alias(value: i32) -> i32 {}

        #[bind_resolved_func(DetachedBorrow)]

        pub unsafe fn borrow(&self) -> &Self {}

        #[bind_resolved_func(DetachedStorage)]

        pub unsafe fn storage<'a>(self: *const Self, out: &'a mut i32) -> &'a mut i32 {}

        #[bind_resolved_func(DetachedGlobalOff)]

        pub unsafe fn global_off(value: i32) -> i32 {}

        #[bind_resolved_func(DetachedZero)]

        pub unsafe fn zero() -> i32 {}

        #[bind_resolved_func(GetPost_Detached)]

        pub unsafe fn request(value: i32) -> i32 {}

        #[cfg(any())]
        #[bind_resolved_func(DoesNotExist)]

        pub unsafe fn excluded() {}
    }
}

use model::Native;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static CONDITION: AtomicBool = AtomicBool::new(false);

static ADD_CALLS: AtomicUsize = AtomicUsize::new(0);

static BORROW_CALLS: AtomicUsize = AtomicUsize::new(0);

static GLOBAL_OFF_CONDITION_CALLS: AtomicUsize = AtomicUsize::new(0);

fn borrow_condition() -> bool {
    assert!(unchained_macros_core::CallbackTarget::physical_detour(Native::__UNCHAINED_TARGET_borrow).is_none());

    let binding = Native::__UNCHAINED_TARGET_borrow.binding();

    assert_eq!(binding.address(), binding.entry_address());

    false
}

fn global_off_condition() -> bool {
    GLOBAL_OFF_CONDITION_CALLS.fetch_add(1, Ordering::SeqCst);

    false
}

#[unchained_macros_core::bind_func_detour(model::Native::add, || CONDITION.load(Ordering::SeqCst))]

unsafe fn plugin_add(value: i32) -> i32 {
    ADD_CALLS.fetch_add(1, Ordering::SeqCst);

    unsafe { Native::add(value) + 100 }
}

#[unchained_macros_core::bind_func_detour(Native::add)]

unsafe fn duplicate(value: i32) -> i32 {
    unsafe { Native::add(value) + 200 }
}

#[unchained_macros_core::bind_func_detour(Native::alias)]

unsafe fn aliased(value: i32) -> i32 {
    unsafe { Native::alias(value) + 300 }
}

#[unchained_macros_core::bind_func_detour(Native::borrow, borrow_condition)]

unsafe fn plugin_borrow(this: &Native) -> &Native {
    BORROW_CALLS.fetch_add(1, Ordering::SeqCst);

    unsafe { this.borrow() }
}

#[unchained_macros_core::bind_func_detour(Native::storage)]

unsafe fn plugin_storage(this: *const Native, out: &mut i32) -> &mut i32 {
    let result = unsafe { this.storage(out) };

    *result += 10;

    result
}

#[unchained_macros_core::bind_func_detour(Native::global_off, global_off_condition)]

unsafe fn plugin_global_off(value: i32) -> i32 {
    unsafe { Native::global_off(value) + 400 }
}

#[unchained_macros_core::bind_func_detour(Native::zero)]

unsafe fn plugin_zero() -> i32 {
    unsafe { Native::zero() + 1 }
}

#[unchained_macros_core::bind_func_detour(Native::request)]

unsafe fn plugin_request(value: i32) -> i32 {
    unsafe { Native::request(value) + 600 }
}

#[cfg(any())]
#[unchained_macros_core::bind_func_detour(DoesNotExist::excluded)]

unsafe fn excluded() {}

#[inline(never)]
unsafe extern "C" fn add_target(value: i32) -> i32 {
    std::hint::black_box(value) + 3
}

#[inline(never)]
unsafe extern "C" fn borrow_target(this: &Native) -> &Native {
    std::hint::black_box(this)
}

#[inline(never)]
unsafe extern "C" fn storage_target(this: *const Native, out: &mut i32) -> &mut i32 {
    *out = unsafe { (*std::hint::black_box(this)).value };

    std::hint::black_box(out)
}

#[inline(never)]
unsafe extern "C" fn global_off_target(value: i32) -> i32 {
    std::hint::black_box(value) + 4
}

#[inline(never)]
unsafe extern "C" fn zero_target() -> i32 {
    std::hint::black_box(17)
}

#[inline(never)]
unsafe extern "C" fn request_target(value: i32) -> i32 {
    std::hint::black_box(value) + 6
}

#[test]

fn detached_replacements_patch_native_entries_but_model_calls_always_use_originals() {
    assert!(std::panic::catch_unwind(|| unsafe { Native::add(1) }).is_err());

    assert!(!unsafe { __UNCHAINED_REGISTRATION_plugin_add.attach() }.unwrap());

    assert!(!unchained_macros_core::detour_is_initialized("plugin_add"));

    assert!(!unchained_macros_core::detour_is_enabled("MissingDetour"));

    assert!(!unchained_macros_core::detour_is_initialized("MissingDetour"));

    for (binding, address) in [
        (
            Native::__UNCHAINED_TARGET_zero.binding(),
            zero_target as *const () as usize,
        ),
        (
            Native::__UNCHAINED_TARGET_request.binding(),
            request_target as *const () as usize,
        ),
        (
            Native::__UNCHAINED_TARGET_add.binding(),
            add_target as *const () as usize,
        ),
        (
            Native::__UNCHAINED_TARGET_alias.binding(),
            add_target as *const () as usize,
        ),
        (
            Native::__UNCHAINED_TARGET_borrow.binding(),
            borrow_target as *const () as usize,
        ),
        (
            Native::__UNCHAINED_TARGET_storage.binding(),
            storage_target as *const () as usize,
        ),
        (
            Native::__UNCHAINED_TARGET_global_off.binding(),
            global_off_target as *const () as usize,
        ),
    ] {
        unsafe {
            binding
                .initialize_address(std::num::NonZeroUsize::new(address).unwrap())
                .unwrap();
        }
    }

    let registration =
        unchained_macros_core::detour_inventory_for::<unchained_macros_patternsleuth::PatternsleuthResolver>()
            .find(|r| r.name() == "plugin_add")
            .unwrap();

    assert_eq!(registration.name(), "plugin_add");

    let binding = Native::__UNCHAINED_TARGET_add.binding();

    assert_eq!(binding.name(), "Native::add");

    assert_eq!(binding.resolver_name(), "DetachedAdd");

    CONDITION.store(true, Ordering::SeqCst);

    unsafe {
        assert_eq!(Native::add(7), 10);

        registration.attach().unwrap();

        __UNCHAINED_REGISTRATION_plugin_borrow.attach().unwrap();

        __UNCHAINED_REGISTRATION_plugin_storage.attach().unwrap();

        assert!(!__UNCHAINED_REGISTRATION_plugin_global_off.attach().unwrap());
        assert_eq!(GLOBAL_OFF_CONDITION_CALLS.load(Ordering::SeqCst), 1);
        __UNCHAINED_REGISTRATION_plugin_zero.attach().unwrap();

        __UNCHAINED_REGISTRATION_plugin_request.attach().unwrap();

        assert_eq!(Native::zero(), 17);

        assert_eq!(std::hint::black_box(zero_target as unsafe extern "C" fn() -> i32)(), 18);

        assert_eq!(Native::request(1), 7);

        assert_eq!(
            std::hint::black_box(request_target as unsafe extern "C" fn(i32) -> i32)(1),
            607
        );

        assert!(unchained_macros_core::detour_is_enabled("plugin_add"));

        assert!(!unchained_macros_core::detour_is_initialized("plugin_borrow"));

        assert!(!unchained_macros_core::detour_is_enabled("plugin_borrow"));

        assert!(!unchained_macros_core::detour_is_enabled("plugin_global_off"));

        let call = std::hint::black_box(add_target as unsafe extern "C" fn(i32) -> i32);

        assert_eq!(call(7), 110);

        assert_eq!(ADD_CALLS.load(Ordering::SeqCst), 1);

        assert_eq!(Native::add(7), 10);

        assert_eq!(Native::alias(7), 10);

        assert_eq!(ADD_CALLS.load(Ordering::SeqCst), 1);

        assert!(__UNCHAINED_REGISTRATION_duplicate.attach().is_err());

        assert!(__UNCHAINED_REGISTRATION_aliased.attach().is_err());

        assert!(__UNCHAINED_REGISTRATION_plugin_add.attach().is_err());

        let native = Native { value: 42 };

        let borrow_call = std::hint::black_box(borrow_target as unsafe extern "C" fn(&Native) -> &Native);

        assert!(std::ptr::eq(borrow_call(&native), &native));

        assert_eq!(BORROW_CALLS.load(Ordering::SeqCst), 0);

        assert!(std::ptr::eq(native.borrow(), &native));
        let storage_call = std::hint::black_box(
            storage_target as for<'a> unsafe extern "C" fn(*const Native, &'a mut i32) -> &'a mut i32,
        );

        let mut out = 0;

        let address = &raw mut out;

        let result = storage_call(&native, &mut out);

        assert_eq!(result as *mut _, address);

        assert_eq!(*result, 52);

        assert_eq!(*(&raw const native).storage(&mut out), 42);

        assert_eq!(Native::global_off(1), 5);

        assert_eq!(
            std::hint::black_box(global_off_target as unsafe extern "C" fn(i32) -> i32)(1),
            5
        );

        let original_address = Native::__UNCHAINED_TARGET_add.binding().address();

        __UNCHAINED_REGISTRATION_plugin_add.detour().unwrap().disable().unwrap();

        assert_eq!(call(7), 10);

        assert!(!unchained_macros_core::detour_is_enabled("plugin_add"));

        assert!(unchained_macros_core::detour_is_initialized("plugin_add"));

        assert_eq!(Native::add(7), 10);

        assert_eq!(Native::__UNCHAINED_TARGET_add.binding().address(), original_address);

        assert!(__UNCHAINED_REGISTRATION_plugin_borrow.detour().is_none());
        __UNCHAINED_REGISTRATION_plugin_storage
            .detour()
            .unwrap()
            .disable()
            .unwrap();

        __UNCHAINED_REGISTRATION_plugin_zero
            .detour()
            .unwrap()
            .disable()
            .unwrap();

        __UNCHAINED_REGISTRATION_plugin_request
            .detour()
            .unwrap()
            .disable()
            .unwrap();
    }
}
