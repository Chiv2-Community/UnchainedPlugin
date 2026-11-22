use std::collections::HashMap;
use unchained_macros_core::{bind_resolved_func, bind_resolved_value, function_is_bound_for, initialize_bindings_for, initialize_value_bindings_where};
use unchained_macros_patternsleuth::{define_pattern_resolver, PatternsleuthResolver, ScanContext};

#[unchained_macros_core::resolver_selection_key(require = [Allow, Deny])]
enum Mode { Allow, Deny }
impl unchained_macros_core::ResolverSelector for Mode {
    fn current() -> Self { Self::Allow }
}

define_pattern_resolver!(Enabled, {
    Mode::Allow: ["AA BB"], Mode::Deny: None,
});
define_pattern_resolver!(Disabled, {
    Mode::Allow: None, Mode::Deny: ["AA BB"],
});
define_pattern_resolver!(EnabledValue, {
    Mode::Allow: ["AA BB"], Mode::Deny: None,
});
define_pattern_resolver!(DisabledValue, {
    Mode::Allow: None, Mode::Deny: ["AA BB"],
});

#[bind_resolved_func(Enabled)]
unsafe fn enabled() -> u32 {}
#[bind_resolved_func(Disabled)]
unsafe fn disabled() -> u32 {}
#[bind_resolved_value(EnabledValue, Value)]
unsafe fn enabled_value() -> u32 {}
#[bind_resolved_value(DisabledValue, Value)]
unsafe fn disabled_value() -> u32 {}

unsafe extern "C" fn fixture() -> u32 { 42 }
static VALUE: u32 = 7;

#[test_case::test_case(false; "missing disabled offsets")]
#[test_case::test_case(true; "stale disabled offsets")]
fn disabled_bindings_are_not_initialized(cached: bool) {
    // Tests share one-time bindings, but always publish identical native addresses.
    let mut offsets = HashMap::from([
        ("Enabled".into(), fixture as *const () as u64 - 1),
        ("EnabledValue".into(), (&raw const VALUE) as u64 - 1),
    ]);
    if cached {
        offsets.insert("Disabled".into(), fixture as *const () as u64 - 1);
        offsets.insert("DisabledValue".into(), (&raw const VALUE) as u64 - 1);
    }
    let snapshot = unchained_macros_core::SelectorSnapshot::from_selection(Mode::Allow).unwrap();
    let applicable = |name: &str| snapshot.supports_resolver_for::<PatternsleuthResolver>(name);
    unsafe {
        initialize_bindings_for::<PatternsleuthResolver>(&ScanContext::new(1, &offsets), &snapshot).unwrap();
        initialize_value_bindings_where(1, &offsets, applicable);
        assert_eq!(enabled(), 42);
        assert_eq!(enabled_value(), 7);
    }
    assert!(function_is_bound_for::<PatternsleuthResolver>("Enabled"));
    assert!(!function_is_bound_for::<PatternsleuthResolver>("Disabled"));
    assert!(std::panic::catch_unwind(|| unsafe { disabled() }).is_err());
    assert!(std::panic::catch_unwind(|| unsafe { disabled_value() }).is_err());
}
