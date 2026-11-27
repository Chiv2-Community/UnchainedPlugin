use std::collections::HashMap;
use unchained_macros_core::{
    bind_resolved_func, function_is_bound_for, initialize_bindings_for, require_function_bindings_for, SelectorSnapshot,
};

use unchained_macros_patternsleuth::{OffsetProvider, PatternsleuthResolver, ScanContext};

unchained_macros_patternsleuth::define_pattern_resolver!(FixtureRead, ["AA"]);

#[bind_resolved_func(FixtureRead)]

unsafe fn read() -> i32 {}

unsafe extern "C" fn fixture() -> i32 {
    42
}

inventory::submit! {
    OffsetProvider {
        name: "FixtureOffsets",
        map: || HashMap::from([("FixtureRead".to_string(), 7)]),
    }
}

#[test]

fn functions_initialize_with_the_host_supplied_base() {
    let snapshot = SelectorSnapshot::capture_registered().unwrap();

    assert!(!function_is_bound_for::<PatternsleuthResolver>("FixtureRead"));

    assert!(require_function_bindings_for::<PatternsleuthResolver>(&["FixtureRead"]).is_err());

    let missing =
        unsafe { initialize_bindings_for::<PatternsleuthResolver>(&ScanContext::new(1, &HashMap::new()), &snapshot) }
            .unwrap_err();

    assert!(missing.contains("FixtureRead"));

    let offsets = HashMap::from([("FixtureRead".to_string(), fixture as *const () as u64 - 1)]);

    let context = ScanContext::new(1, &offsets);

    unsafe {
        initialize_bindings_for::<PatternsleuthResolver>(&context, &snapshot).unwrap();
    }

    unsafe {
        initialize_bindings_for::<PatternsleuthResolver>(&context, &snapshot).unwrap();
    }

    assert!(function_is_bound_for::<PatternsleuthResolver>("FixtureRead"));

    assert!(require_function_bindings_for::<PatternsleuthResolver>(&["FixtureRead"]).is_ok());

    assert!(require_function_bindings_for::<PatternsleuthResolver>(&["Unregistered"]).is_err());

    assert_eq!(unsafe { read() }, 42);

    assert_eq!(
        unchained_macros_core::detour_inventory_for::<unchained_macros_patternsleuth::PatternsleuthResolver>().count(),
        0
    );
}

#[test]

fn offset_providers_use_the_shared_inventory() {
    let registration = inventory::iter::<OffsetProvider>
        .into_iter()
        .find(|registration| registration.name == "FixtureOffsets")
        .unwrap();

    assert_eq!((registration.map)()["FixtureRead"], 7);
}
