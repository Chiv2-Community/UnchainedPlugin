use std::collections::HashMap;
use unchained_macros::{
    bind_resolved_func, function_is_bound, initialize_function_bindings,
    require_function_bindings, HookRegistration, OffsetRegisty,
};

unchained_macros::define_pattern_resolver!(FixtureRead, ["AA"]);

#[bind_resolved_func(FixtureRead)]
unsafe fn read() -> i32 {}

unsafe extern "C" fn fixture() -> i32 { 42 }

inventory::submit! {
    OffsetRegisty {
        name: "FixtureOffsets",
        map: || HashMap::from([("FixtureRead".to_string(), 7)]),
    }
}

#[test]
fn functions_initialize_with_the_host_supplied_base() {
    assert!(!function_is_bound("FixtureRead"));
    assert!(require_function_bindings(&["FixtureRead"]).is_err());
    let missing = unsafe { initialize_function_bindings(1, &HashMap::new()) }.unwrap_err();
    assert!(missing.contains("FixtureRead"));
    let offsets = HashMap::from([("FixtureRead".to_string(), fixture as *const () as u64 - 1)]);
    unsafe { initialize_function_bindings(1, &offsets).unwrap(); }
    unsafe { initialize_function_bindings(1, &offsets).unwrap(); }
    assert!(function_is_bound("FixtureRead"));
    assert!(require_function_bindings(&["FixtureRead"]).is_ok());
    assert!(require_function_bindings(&["Unregistered"]).is_err());
    assert_eq!(unsafe { read() }, 42);
    assert_eq!(inventory::iter::<HookRegistration>.into_iter().count(), 0);
}

#[test]
fn offset_providers_use_the_shared_inventory() {
    let registration = inventory::iter::<OffsetRegisty>.into_iter()
        .find(|registration| registration.name == "FixtureOffsets").unwrap();
    assert_eq!((registration.map)()["FixtureRead"], 7);
}
