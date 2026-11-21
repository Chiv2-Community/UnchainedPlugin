use unchained_macros::{
    define_pattern_resolver, define_selected_pattern_resolver, supports_selection,
};

#[derive(Clone, Copy, PartialEq, Eq, unchained_macros::SelectionKey)]
enum Mode { Read, Write, Shared }

#[derive(Clone, Copy, PartialEq, Eq, unchained_macros::SelectionKey)]
enum Other { Read }

define_selected_pattern_resolver!(Mode::Read, ReadOnly, ["AA BB"]);
define_selected_pattern_resolver!(Mode::Write, WriteOnly, Simple, ["CC DD"]);
define_selected_pattern_resolver!(Mode::Read, ReadCall, Call, ["E8 | ?? ?? ?? ??"]);
define_selected_pattern_resolver!(Mode::Read, ReadInline, ["EE FF"], |ctx, patterns| {
    let matches = ctx.scan(patternsleuth::scanner::Pattern::new(patterns[0]).unwrap()).await;
    patternsleuth::resolvers::ensure_one(matches)
});
define_pattern_resolver!(Unrestricted, ["11 22"]);

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
fn restrictions_apply_across_macro_forms(
    (name, restriction): (&str, Option<Mode>), selection: Mode,
) {
    assert_eq!(supports_selection(name, selection),
        restriction.is_none_or(|expected| expected == selection));
}

#[test]
fn different_enum_types_do_not_match_equal_discriminants() {
    assert!(!supports_selection("ReadOnly", Other::Read));
    assert!(supports_selection("Unrestricted", Other::Read));
}

inventory::submit! {
    unchained_macros::ResolverRestriction {
        name: "Conflicting",
        matches: |key| unchained_macros::__private::selection_matches(key, Mode::Read),
    }
}
inventory::submit! {
    unchained_macros::ResolverRestriction {
        name: "Conflicting",
        matches: |key| unchained_macros::__private::selection_matches(key, Mode::Write),
    }
}

#[test_case::test_case(Mode::Read; "read")]
#[test_case::test_case(Mode::Write; "write")]
#[test_case::test_case(Mode::Shared; "shared")]
fn conflicting_restrictions_fail_closed(selection: Mode) {
    assert!(!supports_selection("Conflicting", selection));
}
