use unchained_macros_core::SelectorSnapshot;
use unchained_macros_patternsleuth::define_pattern_resolver;
use unchained_macros_patternsleuth::PatternsleuthResolver;

#[unchained_macros_core::resolver_selection_key(require = [Read, Write])]
enum Mode {
    Read,
    Write,
    Shared,
}

impl unchained_macros_core::ResolverSelector for Mode {
    fn current() -> Self {
        Self::Read
    }
}

#[unchained_macros_core::resolver_selection_key]
enum Other {
    Read,
}

impl unchained_macros_core::ResolverSelector for Other {
    fn current() -> Self {
        Self::Read
    }
}

#[unchained_macros_core::resolver_selection_key(require = [First])]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum ExistingDerives {
    First = 7,
}

#[unchained_macros_core::resolver_selection_key]
#[derive(Clone)]
enum PartialDerives {
    First,
}

#[test]
fn selection_attribute_preserves_existing_derives_and_discriminants() {
    assert_eq!(ExistingDerives::First as u8, 7);
    assert_eq!(ExistingDerives::First, ExistingDerives::First);
    assert!(PartialDerives::First == PartialDerives::First);
    assert!(<ExistingDerives as unchained_macros_core::SelectionKey>::REQUIRED == [ExistingDerives::First]);
}

define_pattern_resolver!(ReadOnly, {
    Mode::Read: ["AA BB"], Mode::Write: None, Mode::Shared: None,
});

define_pattern_resolver!(WriteOnly, Simple, {
    Mode::Read: None, Mode::Write: ["CC DD"], Mode::Shared: None,
});

define_pattern_resolver!(ReadCall, Call, {
    Mode::Read: ["E8 | ?? ?? ?? ??"], Mode::Write: None, Mode::Shared: None,
});

define_pattern_resolver!(ReadInline, {
    Mode::Read: ["EE FF"], Mode::Write: None, Mode::Shared: None,
}, |ctx, patterns| {
    let matches = ctx.scan(patternsleuth::scanner::Pattern::new(patterns[0]).unwrap()).await;
    patternsleuth::resolvers::ensure_one(matches)
});

define_pattern_resolver!(Unrestricted, ["11 22"]);

use Mode::Read as ReadAlias;
define_pattern_resolver!(AliasedKey, {
    ReadAlias: ["AA BB"], Mode::Write: None,
});

#[test_case::test_matrix(
    [
        ("ReadOnly", Some(Mode::Read)),
        ("WriteOnly", Some(Mode::Write)),
        ("ReadCall", Some(Mode::Read)),
        ("ReadInline", Some(Mode::Read)),
        ("Unrestricted", None),
        ("Unregistered", None),
    ],
    [Mode::Read, Mode::Write, Mode::Shared]
)]
fn restrictions_apply_across_macro_forms((name, restriction): (&str, Option<Mode>), selection: Mode) {
    assert_eq!(
        SelectorSnapshot::from_selection(selection)
            .unwrap()
            .supports_resolver_for::<PatternsleuthResolver>(name),
        restriction.is_none_or(|expected| expected == selection)
    );
}

#[test]
fn required_variants_accept_aliases_and_optional_keys_remain_optional() {
    assert!(<Mode as unchained_macros_core::SelectionKey>::REQUIRED == [Mode::Read, Mode::Write]);
    assert!(SelectorSnapshot::from_selection(Mode::Read)
        .unwrap()
        .supports_resolver_for::<PatternsleuthResolver>("AliasedKey"));
    assert!(!SelectorSnapshot::from_selection(Mode::Shared)
        .unwrap()
        .supports_resolver_for::<PatternsleuthResolver>("AliasedKey"));
    assert!(!SelectorSnapshot::from_selection(Mode::Write)
        .unwrap()
        .supports_resolver_for::<PatternsleuthResolver>("AliasedKey"));
}

#[test]
fn different_enum_types_do_not_match_equal_discriminants() {
    let snapshot = SelectorSnapshot::from_selection(Other::Read).unwrap();
    assert!(!snapshot.supports_resolver_for::<PatternsleuthResolver>("ReadOnly"));
    assert!(snapshot.supports_resolver_for::<PatternsleuthResolver>("Unrestricted"));
}

inventory::submit! {
    &unchained_macros_core::Resolver {
        name: "Conflicting",
        implementation: unchained_macros_patternsleuth::PatternsleuthResolver::named(patternsleuth::resolvers::NamedResolver { name: "Conflicting", getter: ReadOnly::dyn_resolver }),
        filter: Some(unchained_macros_core::ResolverFilter {
        selector: std::any::TypeId::of::<Mode>,
        lookup: |key| unchained_macros_core::__private::dictionary_lookup(key, &[(Mode::Read, true), (Mode::Write, false), (Mode::Shared, false)]),
        }),
    } as &dyn unchained_macros_core::__private::ResolverInfo
}

inventory::submit! {
    &unchained_macros_core::Resolver {
        name: "Conflicting",
        implementation: unchained_macros_patternsleuth::PatternsleuthResolver::named(patternsleuth::resolvers::NamedResolver { name: "Conflicting", getter: ReadOnly::dyn_resolver }),
        filter: Some(unchained_macros_core::ResolverFilter {
        selector: std::any::TypeId::of::<Mode>,
        lookup: |key| unchained_macros_core::__private::dictionary_lookup(key, &[(Mode::Read, false), (Mode::Write, true), (Mode::Shared, false)]),
        }),
    } as &dyn unchained_macros_core::__private::ResolverInfo
}

#[test_case::test_case(Mode::Read; "read")]
#[test_case::test_case(Mode::Write; "write")]
#[test_case::test_case(Mode::Shared; "shared")]
fn conflicting_restrictions_fail_closed(selection: Mode) {
    assert!(!SelectorSnapshot::from_selection(selection)
        .unwrap()
        .supports_resolver_for::<PatternsleuthResolver>("Conflicting"));
}
