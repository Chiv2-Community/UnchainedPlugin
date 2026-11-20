use std::collections::HashMap;
use unchained_macros::{
    bind_resolved_func, function_is_bound, initialize_function_bindings,
    require_function_bindings, HookRegistration, OffsetRegisty, PatchRegistration,
    PlatformType, PLATFORM,
};

struct FixtureRead;

#[bind_resolved_func(FixtureRead)]
unsafe fn read() -> i32 {}

unsafe extern "C" fn fixture() -> i32 { 42 }

// Only the memory-write backend remains consumer-owned.
mod tools {
    pub mod memtools {
        pub unsafe fn patch(address: *mut u8, bytes: &[u8]) {
            unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), address, bytes.len()); }
        }
    }
}

unchained_macros::CREATE_PATCH_PLATFORM!(STEAM, FixturePatch, BYTES, &[0x90]);

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
fn patches_use_shared_registration_and_the_host_supplied_base() {
    let registration = inventory::iter::<PatchRegistration>.into_iter()
        .find(|registration| registration.name == "FixturePatch").unwrap();
    assert!(!(registration.enabled_fn)());
    PLATFORM.set(PlatformType::STEAM).unwrap();
    assert!((registration.enabled_fn)());
    let mut bytes = [0u8; 4];
    let base = bytes.as_mut_ptr() as usize;
    let offsets = HashMap::from([("FixturePatch".to_string(), 2)]);
    unsafe { (registration.patch_fn)(base, offsets).unwrap(); }
    assert_eq!(bytes, [0, 0, 0x90, 0]);
}

#[test]
fn offset_providers_use_the_shared_inventory() {
    let registration = inventory::iter::<OffsetRegisty>.into_iter()
        .find(|registration| registration.name == "FixtureOffsets").unwrap();
    assert_eq!((registration.map)()["FixtureRead"], 7);
}
