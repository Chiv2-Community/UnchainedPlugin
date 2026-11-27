//! Compile reflection bindings without generated resolver marker types.
#![allow(non_snake_case, non_camel_case_types, non_upper_case_globals, dead_code)]

use unchained_macros_core as core;
use unchained_macros_kismet::{kismet_runtime_bindings, KismetExecResolver, KismetReflectionResolver};

struct UKismetSystemLibrary;

// These authored names must not conflict with generated module-level types.
enum UKismetSystemLibrary__IsDedicatedServer {}

enum UKismetSystemLibrary__IsServer {}

enum UKismetSystemLibrary__IsStandalone {}

#[kismet_runtime_bindings("/Script/Engine.KismetSystemLibrary")]

impl UKismetSystemLibrary {
    const __UNCHAINED_KISMET_RESOLVER_IsServer: () = ();

    pub(crate) const AUTHORED: u32 = 42;

    pub(crate) unsafe fn IsDedicatedServer(value: *const ()) -> bool {}

    pub unsafe fn IsServer(value: *const ()) -> bool {}

    unsafe fn IsStandalone(value: *const ()) -> bool {}

    #[cfg(any())]

    unsafe fn Excluded(value: MissingType) {}

    #[dummy]

    pub(crate) unsafe fn Observed(value: i32) {}

    #[cfg(any())]
    #[dummy]

    unsafe fn ExcludedObservation(value: bool) {}

    #[cfg_attr(all(), cfg(any()))]
    #[dummy]

    unsafe fn ExcludedByAttribute() {}
}

struct Borrowed;

#[kismet_runtime_bindings("/Script/Fixture.Borrowed")]

impl Borrowed {
    unsafe fn identity<'a>(value: &'a i32) -> &'a i32 {}

    unsafe fn receiver(&self) -> &Self {}

    #[dummy]

    unsafe fn Observed(value: i32) {}
}

struct OtherLibrary;

#[kismet_runtime_bindings("/Script/Fixture.OtherLibrary")]

impl OtherLibrary {
    unsafe fn IsServer(value: *const ()) -> bool {}

    #[dummy]

    unsafe fn Observed(value: i32) {}
}

#[kismet_runtime_bindings("/Script/Fixture.Excluded")]
#[cfg(any())]

impl MissingType {
    unsafe fn excluded(value: MissingType) {}
}

unsafe extern "C" fn identity(value: &i32) -> &i32 {

    value
}

#[test]

fn reflection_descriptors_preserve_names_backend_signatures_and_lifetimes() {

    let mut names: Vec<_> = core::resolver_inventory_for::<KismetReflectionResolver>()
        .map(|resolver| resolver.name)
        .collect();

    names.sort_unstable();

    assert_eq!(
        names,
        [
            "Borrowed__identity",
            "Borrowed__receiver",
            "OtherLibrary__IsServer",
            "UKismetSystemLibrary__IsDedicatedServer",
            "UKismetSystemLibrary__IsServer",
            "UKismetSystemLibrary__IsStandalone",
        ]
    );

    assert_eq!(core::binding_inventory_for::<KismetReflectionResolver>().count(), 6);

    let mut dummy_names: Vec<_> = core::resolver_inventory_for::<KismetExecResolver>()
        .map(|resolver| resolver.name)
        .collect();

    dummy_names.sort_unstable();

    assert_eq!(
        dummy_names,
        [
            "Borrowed__Observed__exec",
            "OtherLibrary__Observed__exec",
            "UKismetSystemLibrary__Observed__exec",
        ]
    );

    assert_eq!(core::binding_inventory_for::<KismetExecResolver>().count(), 3);

    assert_eq!(UKismetSystemLibrary::AUTHORED, 42);

    let _: () = UKismetSystemLibrary::Observed;

    let _: &core::NativeTarget<unsafe extern "C" fn(*const ()) -> bool, _, _, _, KismetReflectionResolver> =
        UKismetSystemLibrary::__UNCHAINED_TARGET_IsDedicatedServer;

    let _: &core::NativeTarget<for<'a> unsafe extern "C" fn(&'a i32) -> &'a i32, _, _, _, KismetReflectionResolver> =
        Borrowed::__UNCHAINED_TARGET_identity;

    let binding = Borrowed::__UNCHAINED_TARGET_identity.binding();

    unsafe {

        binding
            .initialize_address(std::num::NonZeroUsize::new(identity as *const () as usize).unwrap())
            .unwrap();

        let value = 42;

        assert!(std::ptr::eq(Borrowed::identity(&value), &value));
    }
}
