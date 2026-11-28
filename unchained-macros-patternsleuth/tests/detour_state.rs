use unchained_macros_core::{detour_is_enabled, detour_is_initialized, DetourInfo, DetourRegistration, NativeTarget};

static BINDING: unchained_macros_core::FunctionBinding<unchained_macros_patternsleuth::PatternsleuthResolver> =
    unchained_macros_core::FunctionBinding::new(
        "Fixture::target",
        &unchained_macros_core::Resolver {
            name: "FixtureTarget",
            implementation: unchained_macros_patternsleuth::PatternsleuthResolver::offset("FixtureTarget"),
            filter: None,
        },
    );

inventory::submit! { &BINDING as &dyn unchained_macros_core::__private::FunctionInfo }

#[inline(never)]
unsafe extern "C" fn target(value: i32) -> i32 {
    unsafe { std::ptr::read_volatile(&value) }.wrapping_add(1)
}

unsafe extern "C" fn replacement(value: i32) -> i32 {
    value + 2
}

unsafe extern "C" fn dispatch(value: i32) -> i32 {
    let call: unsafe extern "C" fn(i32) -> i32 = unchained_macros_core::SupportsDetour::replacement(&TARGET)
        .unwrap_or_else(|| unsafe { std::mem::transmute(BINDING.address().unwrap().get()) });

    unsafe { call(value) }
}

type Target =
    NativeTarget<unsafe extern "C" fn(i32) -> i32, (), (), unchained_macros_patternsleuth::PatternsleuthResolver>;

static TARGET: Target = unsafe { Target::new(&BINDING, dispatch, dispatch, dispatch, |f| f as *const ()) };

static REGISTRATION: DetourRegistration<Target> =
    DetourRegistration::new("FixtureDetour", &TARGET, replacement, || true);

inventory::submit! { &REGISTRATION as &dyn DetourInfo }

#[test]

fn state_queries_share_hook_registration_and_track_disabled_trampolines() {
    assert!(!detour_is_initialized("Unknown"));

    assert!(!detour_is_enabled("Unknown"));

    assert!(!detour_is_initialized("FixtureDetour"));

    assert!(!detour_is_enabled("FixtureDetour"));

    unsafe {
        BINDING
            .initialize_address(std::num::NonZeroUsize::new(target as *const () as usize).unwrap())
            .unwrap();

        REGISTRATION.attach(false).unwrap();
    }

    assert!(detour_is_initialized("FixtureDetour"));

    assert!(!detour_is_enabled("FixtureDetour"));

    let call = std::hint::black_box(target as unsafe extern "C" fn(i32) -> i32);

    assert_eq!(unsafe { call(3) }, 4);

    REGISTRATION.set_enabled(true);

    assert!(detour_is_enabled("FixtureDetour"));

    assert_eq!(unsafe { call(3) }, 5);

    unsafe {
        REGISTRATION.detour().unwrap().disable().unwrap();
    }

    assert!(detour_is_initialized("FixtureDetour"));

    assert!(!detour_is_enabled("FixtureDetour"));

    assert_eq!(unsafe { call(3) }, 4);
}
