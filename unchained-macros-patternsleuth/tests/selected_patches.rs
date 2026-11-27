use std::cell::Cell;
use std::collections::HashMap;
use unchained_macros_core::{
    apply_patches_for, register_patch, validate_patch_registrations, Patch, PatchRegistration, PatchState, PatchWriter,
};

use unchained_macros_patternsleuth::define_pattern_resolver;
use unchained_macros_patternsleuth::{PatternsleuthResolver, ScanContext};

#[unchained_macros_core::resolver_selection_key(require = [First, Second])]

enum Key {
    First,
    Second,
    Optional,
}

impl unchained_macros_core::ResolverSelector for Key {
    fn current() -> Self {

        Self::First
    }
}

thread_local! { static FACTORIES: Cell<usize> = const { Cell::new(0) }; }

define_pattern_resolver!(Universal, ["AA BB"]);

define_pattern_resolver!(RootOnly, ["AA BB"]);

define_pattern_resolver!(Conditional, ["AA BB"]);

define_pattern_resolver!(Restricted, {
    Key::First: ["AA BB"], Key::Second: None, Key::Optional: None,
});

#[register_patch(Universal)]

static SHARED: Patch = Patch::bytes(1, &[0x77]);

#[register_patch(Universal, Key::First)]

static FIRST: Patch = Patch::bytes(2, &[0xAA]);

#[register_patch(Universal, Key::Second)]

fn second(resolved_address: u64) -> Patch {

    FACTORIES.set(FACTORIES.get() + 1);

    Patch::owned_bytes(2, resolved_address.to_le_bytes().to_vec())
}

#[register_patch(Restricted, Key::First)]

static ONLY_FIRST: Patch = Patch::nop(3, 2);

#[register_patch(RootOnly, Key::First)]

static ROOT_FIRST: Patch = Patch::bytes(0, &[1]).when(false);

#[register_patch(RootOnly, Key::Second)]

static ROOT_SECOND: Patch = Patch::bytes(0, &[2]).when(false);

#[register_patch(Conditional)]

fn conditional(_resolved_address: u64) -> Patch {

    FACTORIES.set(FACTORIES.get() + 1);

    Patch::bytes(0, &[0xEE]).when(false)
}

use Universal as ImportedResolver;

#[register_patch(ImportedResolver)]

static ALIAS: Patch = Patch::bytes(4, &[0x88]);

#[register_patch(Universal)]

static FLOAT_WRITE: Patch = Patch::write_ref(2, &1.5f32);

#[register_patch(Universal)]

fn word_write(_resolved_address: u64) -> Patch {

    FACTORIES.set(FACTORIES.get() + 1);

    Patch::write(2, 0x12345678u32)
}

#[derive(Default)]

struct Writer(Vec<(u64, Vec<u8>)>);

impl PatchWriter for Writer {
    unsafe fn bytes(&mut self, address: u64, bytes: &[u8]) -> Result<(), String> {

        self.0.push((address, bytes.to_vec()));

        Ok(())
    }

    unsafe fn nop(&mut self, address: u64, count: usize) -> Result<(), String> {

        self.0.push((address, vec![0x90; count]));

        Ok(())
    }

    unsafe fn write(&mut self, address: u64, value: &unchained_macros_core::WriteValue) -> Result<(), String> {

        // This fixture records initialized scalar values only.
        let bytes = unsafe {

            std::slice::from_raw_parts(value.as_ptr(), value.size())
        };

        self.0.push((address, bytes.to_vec()));

        Ok(())
    }
}

fn registration(name: &str) -> &'static PatchRegistration<PatternsleuthResolver> {

    unchained_macros_core::patch_inventory_for::<unchained_macros_patternsleuth::PatternsleuthResolver>()
        .find(|patch| patch.name == name)
        .unwrap()
}

#[test]

fn statics_and_factories_use_patch_names_not_resolver_names() {

    assert_eq!(registration("ALIAS").resolver.name, "Universal");

    assert_eq!(registration("SHARED").resolver.name, "Universal");

    assert!(std::ptr::eq(
        registration("ALIAS").resolver,
        registration("SHARED").resolver
    ));

    assert!(std::ptr::eq(
        registration("ALIAS").resolver,
        <Universal as unchained_macros_core::ResolverDeclaration>::DESCRIPTOR
    ));

    let mut writer = Writer::default();

    unsafe {

        (registration("SHARED").factory)(100).apply(100, &mut writer).unwrap();

        (registration("second").factory)(200).apply(200, &mut writer).unwrap();

        (registration("ONLY_FIRST").factory)(300)
            .apply(300, &mut writer)
            .unwrap();
    }

    assert_eq!(
        writer.0,
        [
            (101, vec![0x77]),
            (202, 200u64.to_le_bytes().to_vec()),
            (303, vec![0x90; 2]),
        ]
    );
}

#[test_case::test_case(Key::First, true; "first")]
#[test_case::test_case(Key::Second, true; "second")]
#[test_case::test_case(Key::Optional, false; "optional has no patch consumer")]

fn selection_filtering_and_roots_do_not_construct_patches(key: Key, root_required: bool) {

    FACTORIES.set(0);

    validate_patch_registrations().unwrap();

    let roots = unchained_macros_patternsleuth::startup_resolvers(
        &unchained_macros_core::SelectorSnapshot::from_selection(key).unwrap(),
    )
    .unwrap();

    assert_eq!(roots.iter().any(|resolver| resolver.name == "RootOnly"), root_required);

    assert_eq!(FACTORIES.get(), 0);

    assert!((registration("SHARED").applies_to)(&key));

    assert_eq!((registration("ONLY_FIRST").applies_to)(&key), key == Key::First);

    assert!(unchained_macros_core::SelectorSnapshot::from_selection(key)
        .unwrap()
        .supports_resolver_for::<PatternsleuthResolver>("Universal"));
}

#[test_case::test_case(Key::First, 2; "first")]
#[test_case::test_case(Key::Second, 3; "second")]
#[test_case::test_case(Key::Optional, 2; "optional")]

fn registered_application_selects_named_patches_and_preserves_conditions(key: Key, expected_factories: usize) {

    FACTORIES.set(0);

    let offsets = HashMap::from([
        ("Universal".into(), 10),
        ("RootOnly".into(), 20),
        ("Conditional".into(), 30),
        ("Restricted".into(), 40),
    ]);

    let mut writer = Writer::default();

    let snapshot = unchained_macros_core::SelectorSnapshot::from_selection(key).unwrap();

    let results = unsafe {

        apply_patches_for::<PatternsleuthResolver, _>(
            &ScanContext::new(1000, &offsets),
            &snapshot,
            &mut writer,
            &mut PatchState::default(),
        )
    }
    .unwrap();

    assert!(results.iter().any(|result| result.name == "SHARED" && result.applied));

    assert!(results
        .iter()
        .any(|result| result.name == "conditional" && !result.applied));

    assert_eq!(
        results.iter().any(|result| result.name == "ONLY_FIRST"),
        key == Key::First
    );

    assert_eq!(results.iter().any(|result| result.name == "second"), key == Key::Second);

    assert_eq!(FACTORIES.get(), expected_factories);

    assert!(writer.0.contains(&(1011, vec![0x77])));

    assert!(writer.0.contains(&(1012, 1.5f32.to_ne_bytes().to_vec())));

    assert!(writer.0.contains(&(1012, 0x12345678u32.to_ne_bytes().to_vec())));

    assert!(!writer.0.iter().any(|(address, _)| *address == 1030));
}

#[test]

fn missing_addresses_fail_before_factories_or_writes() {

    FACTORIES.set(0);

    let mut writer = Writer::default();

    let snapshot = unchained_macros_core::SelectorSnapshot::from_selection(Key::First).unwrap();

    let error = unsafe {

        apply_patches_for::<PatternsleuthResolver, _>(
            &ScanContext::new(1000, &HashMap::new()),
            &snapshot,
            &mut writer,
            &mut PatchState::default(),
        )
    }
    .unwrap_err();

    assert!(error.contains("No address for patch"), "{error}");

    assert_eq!(FACTORIES.get(), 0);

    assert!(writer.0.is_empty());
}

#[test_case::test_case(false; "disabled skips overflow")]
#[test_case::test_case(true; "enabled rejects overflow")]

fn relative_address_overflow_never_writes(enabled: bool) {

    let mut writer = Writer::default();

    let patch = Patch::bytes(1, &[1]).when(enabled);

    let result = unsafe {

        patch.apply(u64::MAX, &mut writer)
    };

    if enabled {

        assert!(result.is_err());
    } else {

        assert_eq!(result.unwrap(), false);
    }

    assert!(writer.0.is_empty());
}
