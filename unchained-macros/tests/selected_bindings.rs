use std::collections::HashMap;
use unchained_macros::{
    bind_resolved_func, bind_resolved_value, define_pattern_resolver,
    function_is_bound, initialize_function_bindings_where,
    initialize_value_bindings_where, supports_selection,
};

#[unchained_macros::resolver_selection_key(require = [Allow, Deny])]
enum Mode { Allow, Deny }

define_pattern_resolver!(Enabled, select [Mode::Allow], {
    Mode::Allow: ["AA BB"], Mode::Deny: None,
});
define_pattern_resolver!(Disabled, select [Mode::Allow], {
    Mode::Allow: None, Mode::Deny: ["AA BB"],
});
define_pattern_resolver!(EnabledValue, select [Mode::Allow], {
    Mode::Allow: ["AA BB"], Mode::Deny: None,
});
define_pattern_resolver!(DisabledValue, select [Mode::Allow], {
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
    let applicable = |name: &str| supports_selection(name, Mode::Allow);
    unsafe {
        initialize_function_bindings_where(1, &offsets, applicable).unwrap();
        initialize_value_bindings_where(1, &offsets, applicable);
        assert_eq!(enabled(), 42);
        assert_eq!(enabled_value(), 7);
    }
    assert!(function_is_bound("Enabled"));
    assert!(!function_is_bound("Disabled"));
    assert!(std::panic::catch_unwind(|| unsafe { disabled() }).is_err());
    assert!(std::panic::catch_unwind(|| unsafe { disabled_value() }).is_err());
}
