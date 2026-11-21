//! Consumer-defined selection keys and resolver restrictions.

use std::any::Any;

/// A fieldless enum used to select resolver behavior.
///
/// Use [`crate::resolver_selection_key`] to validate the declaration and implement
/// this trait. Keys have no implicit default,
/// shared variant, or process-global state.
/// `#[resolver_selection_key(require = [Variant, ...])]` exposes typed [`Self::REQUIRED`]
/// metadata and requires those keys in every enum-keyed resolver dictionary.
///
/// ```
/// #[unchained_macros::resolver_selection_key]
/// enum Mode { Read, Write }
/// assert!(unchained_macros::supports_selection("unrestricted", Mode::Read));
/// ```
///
/// ```
/// #[unchained_macros::resolver_selection_key(require = [Read, Write])]
/// enum Mode { Read, Write, Shared }
/// unchained_macros::define_pattern_resolver!(ReadOnly, select Mode; [Mode::Read], {
///     Mode::Read: ["AA BB"], Mode::Write: None,
/// });
/// fn main() {
///     assert!(<Mode as unchained_macros::SelectionKey>::REQUIRED == [Mode::Read, Mode::Write]);
/// }
/// ```
///
/// Payloads and generics are rejected:
/// ```compile_fail
/// #[unchained_macros::resolver_selection_key]
/// enum Mode { Value(u8) }
/// ```
/// ```compile_fail
/// #[unchained_macros::resolver_selection_key]
/// enum Mode<const N: usize> { Value }
/// ```
pub trait SelectionKey: Copy + Eq + Send + Sync + 'static {
    /// Keys every dictionary must explicitly cover with patterns or `None`.
    /// Configure through `#[resolver_selection_key(require = [Variant, ...])]`.
    const REQUIRED: &'static [Self] = &[];
    fn name(self) -> &'static str;
}

/// Compile-time resolver identity and applicability.
pub trait ResolverDeclaration {
    const NAME: &'static str;
    const UNRESTRICTED: bool;
}

/// Typed applicability metadata. Empty policies mean unrestricted coverage.
pub trait ResolverSelection<E: SelectionKey>: ResolverDeclaration {
    const SELECTIONS: &'static [(E, bool)];
}

#[doc(hidden)]
pub const fn resolver_selections<R: ResolverSelection<E>, E: SelectionKey>(_: E) -> &'static [(E, bool)] {
    R::SELECTIONS
}

/// Restrict a resolver and its hooks/patches by a typed selection key.
pub struct ResolverRestriction {
    pub name: &'static str,
    pub matches: fn(&dyn Any) -> bool,
}

inventory::collect!(ResolverRestriction);

/// Unrestricted names are accepted. All restrictions must match; a restriction
/// for a different enum fails closed rather than comparing discriminants.
pub fn supports_selection<E: SelectionKey>(name: &str, selection: E) -> bool {
    inventory::iter::<ResolverRestriction>.into_iter()
        .filter(|restriction| restriction.name == name)
        .all(|restriction| (restriction.matches)(&selection))
}

#[doc(hidden)]
pub fn dictionary_allows<E: SelectionKey>(selection: &dyn Any, entries: &[(E, bool)]) -> bool {
    selection.downcast_ref::<E>().is_some_and(|value|
        entries.iter().find(|(key, _)| key == value).is_none_or(|(_, enabled)| *enabled))
}

#[doc(hidden)]
pub fn selection_matches<E: SelectionKey>(selection: &dyn Any, expected: E) -> bool {
    selection.downcast_ref::<E>().is_some_and(|value| *value == expected)
}

#[doc(hidden)]
pub fn selection_key<E: SelectionKey>(selection: E) -> E {
    selection
}
