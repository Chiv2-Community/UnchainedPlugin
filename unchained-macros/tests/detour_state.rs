use std::sync::atomic::{AtomicBool, Ordering};
use unchained_macros::{detour_is_enabled, detour_is_initialized, HookRegistration};

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static ENABLED: AtomicBool = AtomicBool::new(false);

inventory::submit! {
    HookRegistration {
        name: "FixtureDetour",
        hook_fn: |_, _| unreachable!(),
        condition: || true,
        initialized: || INITIALIZED.load(Ordering::SeqCst),
        enabled: || ENABLED.load(Ordering::SeqCst),
    }
}

#[test]
fn state_queries_share_hook_registration_and_track_disabled_trampolines() {
    assert!(!detour_is_initialized("Unknown"));
    assert!(!detour_is_enabled("Unknown"));
    assert!(!detour_is_initialized("FixtureDetour"));
    assert!(!detour_is_enabled("FixtureDetour"));
    INITIALIZED.store(true, Ordering::SeqCst);
    assert!(detour_is_initialized("FixtureDetour"));
    assert!(!detour_is_enabled("FixtureDetour"));
    ENABLED.store(true, Ordering::SeqCst);
    assert!(detour_is_enabled("FixtureDetour"));
    ENABLED.store(false, Ordering::SeqCst);
    assert!(detour_is_initialized("FixtureDetour"));
    assert!(!detour_is_enabled("FixtureDetour"));
}
