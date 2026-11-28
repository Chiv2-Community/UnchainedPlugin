use unchained_macros_core::{detour_is_attached, DetourInfo, DetourRegistration, NativeTarget};

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

type Target =
    NativeTarget<unsafe extern "C" fn(i32) -> i32, (), (), unchained_macros_patternsleuth::PatternsleuthResolver>;

static TARGET: Target = unsafe { Target::new(&BINDING, replacement, replacement, |f| f as *const ()) };

static REGISTRATION: DetourRegistration<Target> =
    DetourRegistration::new("FixtureDetour", &TARGET, replacement, || true);

inventory::submit! { &REGISTRATION as &dyn DetourInfo }

#[test]

fn attachment_status_changes_only_after_installation() {
    assert!(!detour_is_attached("Unknown"));

    assert!(!detour_is_attached("FixtureDetour"));

    unsafe {
        BINDING
            .initialize_address(std::num::NonZeroUsize::new(target as *const () as usize).unwrap())
            .unwrap();

        assert!(REGISTRATION.attach().unwrap());
    }
    let call = std::hint::black_box(target as unsafe extern "C" fn(i32) -> i32);
    assert!(detour_is_attached("FixtureDetour"));

    assert_eq!(unsafe { call(3) }, 5);
}
