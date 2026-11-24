use unchained_macros_core::{detour_is_enabled, detour_is_initialized, HookRegistration, NativeHook};

static HOOK: NativeHook = NativeHook::new();
static BINDING: unchained_macros_core::FunctionBinding<unchained_macros_patternsleuth::PatternsleuthResolver> =
    unchained_macros_core::FunctionBinding::new("Fixture::target", &unchained_macros_core::Resolver {
        name: "FixtureTarget", implementation: unchained_macros_patternsleuth::PatternsleuthResolver::offset("FixtureTarget"), filter: None,
    });
inventory::submit! { &BINDING as &dyn unchained_macros_core::__private::FunctionInfo }

#[inline(never)]
unsafe extern "C" fn target(value: i32) -> i32 {
    unsafe { std::ptr::read_volatile(&value) }.wrapping_add(1)
}
unsafe extern "C" fn replacement(value: i32) -> i32 { value + 2 }
unsafe extern "C" fn dispatch(value: i32) -> i32 {
    let address = HOOK.replacement_address().unwrap_or_else(|| BINDING.address().unwrap().get());
    let call: unsafe extern "C" fn(i32) -> i32 = unsafe { std::mem::transmute(address) };
    unsafe { call(value) }
}

static REGISTRATION: HookRegistration = HookRegistration::new(
    "FixtureDetour", &BINDING, &HOOK,
    |activate| unsafe {
        if HOOK.has_replacement() { return Err("duplicate replacement".into()); }
        HOOK.attach(&BINDING, dispatch as *const ())?;
        HOOK.register_replacement(replacement as *const () as usize, activate)?;
        Ok(())
    },
    || true,
);
inventory::submit! { &REGISTRATION }

#[test]
fn state_queries_share_hook_registration_and_track_disabled_trampolines() {
    assert!(!detour_is_initialized("Unknown"));
    assert!(!detour_is_enabled("Unknown"));
    assert!(!detour_is_initialized("FixtureDetour"));
    assert!(!detour_is_enabled("FixtureDetour"));
    unsafe {
        BINDING.initialize_address(std::num::NonZeroUsize::new(target as *const () as usize).unwrap()).unwrap();
        REGISTRATION.attach(false).unwrap();
    }
    assert!(detour_is_initialized("FixtureDetour"));
    assert!(!detour_is_enabled("FixtureDetour"));
    let call = std::hint::black_box(target as unsafe extern "C" fn(i32) -> i32);
    assert_eq!(unsafe { call(3) }, 4);
    REGISTRATION.set_enabled(true);
    assert!(detour_is_enabled("FixtureDetour"));
    assert_eq!(unsafe { call(3) }, 5);
    unsafe { REGISTRATION.detour().unwrap().disable().unwrap(); }
    assert!(detour_is_initialized("FixtureDetour"));
    assert!(!detour_is_enabled("FixtureDetour"));
    assert_eq!(unsafe { call(3) }, 4);
}
