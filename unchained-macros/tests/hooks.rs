#[test_case::test_case(unchained_macros::__create_hook_impl!(@as_cond ACTIVE) as fn() -> bool, true; "active")]
#[test_case::test_case(unchained_macros::__create_hook_impl!(@as_cond INACTIVE) as fn() -> bool, false; "inactive")]
#[test_case::test_case(unchained_macros::__create_hook_impl!(@as_cond CALLED) as fn() -> bool, false; "called")]
#[test_case::test_case(unchained_macros::__create_hook_impl!(@as_cond DISABLED) as fn() -> bool, false; "disabled")]
fn hook_status_preserves_activation_condition(condition: fn() -> bool, enabled: bool) {
    assert_eq!(condition(), enabled);
}

#[test]
fn explicit_hook_condition_is_evaluated_when_called() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let enabled = AtomicBool::new(false);
    let condition = unchained_macros::__create_hook_impl!(@as_cond { || enabled.load(Ordering::Relaxed) });
    assert!(!condition());
    enabled.store(true, Ordering::Relaxed);
    assert!(condition());
}

#[test]
fn none_dispatch_returns_body_value() {
    let result = unchained_macros::__hook_dispatch!(NONE, NotAttached, i32, (0), { 42 });
    assert_eq!(result, 42);
}
