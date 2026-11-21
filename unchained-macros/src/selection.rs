//! Consumer-defined selection keys and resolver restrictions.

use std::any::Any;

/// A fieldless enum used to select resolver behavior.
///
/// Derive this trait to validate the declaration. Keys have no implicit default,
/// shared variant, or process-global state.
///
/// ```
/// #[derive(Clone, Copy, PartialEq, Eq, unchained_macros::SelectionKey)]
/// enum Mode { Read, Write }
/// assert!(unchained_macros::supports_selection("unrestricted", Mode::Read));
/// ```
///
/// Payloads and generics are rejected:
/// ```compile_fail
/// #[derive(Clone, Copy, PartialEq, Eq, unchained_macros::SelectionKey)]
/// enum Mode { Value(u8) }
/// ```
/// ```compile_fail
/// #[derive(Clone, Copy, PartialEq, Eq, unchained_macros::SelectionKey)]
/// enum Mode<const N: usize> { Value }
/// ```
pub trait SelectionKey: Copy + Eq + Send + Sync + 'static {}

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
pub fn selection_matches<E: SelectionKey>(selection: &dyn Any, expected: E) -> bool {
    selection.downcast_ref::<E>().is_some_and(|value| *value == expected)
}

#[doc(hidden)]
pub fn selection_key<E: SelectionKey>(selection: E) -> E {
    selection
}

/// Restrict a resolver and its hooks/patches to a consumer-defined enum variant.
#[macro_export]
macro_rules! define_selected_pattern_resolver {
    ($selection:expr, $name:ident, $($resolver:tt)+) => {
        $crate::__private::inventory::submit! {
            $crate::ResolverRestriction {
                name: stringify!($name),
                matches: |key| $crate::__private::selection_matches(key, $selection),
            }
        }
        $crate::define_pattern_resolver!($name, $($resolver)+);
    };
}
