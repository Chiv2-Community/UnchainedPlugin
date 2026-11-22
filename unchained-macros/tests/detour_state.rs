use std::sync::OnceLock;
use unchained_macros::{detour_is_enabled, detour_is_initialized, HookRegistration};

static HOOK: OnceLock<retour::RawDetour> = OnceLock::new();
static BINDING: unchained_macros::FunctionBinding =
    unchained_macros::FunctionBinding::new("Fixture::target", &unchained_macros::ResolverDescriptor {
        name: "FixtureTarget", implementation: unchained_macros::PatternsleuthResolver::offset("FixtureTarget"), filter: None,
    });

#[inline(never)]
unsafe extern "C" fn target(value: i32) -> i32 {
    unsafe { std::ptr::read_volatile(&value) }.wrapping_add(1)
}

unsafe extern "C" fn replacement(value: i32) -> i32 { value + 2 }

inventory::submit! {
    unchained_macros::__private::ErasedHookRegistration::new(&HookRegistration {
        name: "FixtureDetour",
        binding: &BINDING,
        hook_fn: |_| unreachable!(),
        condition: || true,
        detour: || HOOK.get(),
    })
}

#[test]
fn state_queries_share_hook_registration_and_track_disabled_trampolines() {
    assert!(!detour_is_initialized("Unknown"));
    assert!(!detour_is_enabled("Unknown"));
    assert!(!detour_is_initialized("FixtureDetour"));
    assert!(!detour_is_enabled("FixtureDetour"));
    HOOK.set(unsafe { retour::RawDetour::new(target as *const (), replacement as *const ()).unwrap() }).unwrap();
    assert!(detour_is_initialized("FixtureDetour"));
    assert!(!detour_is_enabled("FixtureDetour"));
    unsafe { HOOK.get().unwrap().enable().unwrap(); }
    assert!(detour_is_enabled("FixtureDetour"));
    unsafe { HOOK.get().unwrap().disable().unwrap(); }
    assert!(detour_is_initialized("FixtureDetour"));
    assert!(!detour_is_enabled("FixtureDetour"));
}
