//! Startup roots are consumers, not every resolver linked by a dependency.

use std::collections::BTreeSet;
use patternsleuth::resolvers::NamedResolver;
use crate::{FunctionBindingRegistration, ValueBindingRegistration, PatchRegistration};

/// A resolver declared by this plugin. Prefer it over an upstream namesake.
pub struct DeclaredResolver {
    pub resolver: NamedResolver,
}

/// An explicit scan consumer beyond the binding and patch inventories.
pub struct RequiredResolver {
    pub name: &'static str,
}

inventory::collect!(DeclaredResolver);
inventory::collect!(RequiredResolver);

/// Require a resolver whose scan supplies data beyond a direct native binding.
///
/// Bindings and instruction patches register their own roots automatically.
#[macro_export]
macro_rules! require_resolver {
    ($name:ident) => {
        $crate::__private::inventory::submit! {
            $crate::RequiredResolver { name: stringify!($name) }
        }
    };
}

/// Select only required startup roots. Their transitive dependencies remain
/// resolved by Patternsleuth's shared context. Missing/ambiguous roots fail closed.
pub fn startup_resolvers() -> Result<Vec<&'static NamedResolver>, String> {
    startup_resolvers_matching(|_| true)
}

/// Select scan consumers applicable to a host-defined key. Runtime patch
/// conditions are deliberately not evaluated while selecting roots.
pub fn startup_resolvers_for<E: crate::SelectionKey>(selection: E) -> Result<Vec<&'static NamedResolver>, String> {
    startup_resolvers_matching(|patch| (patch.applies_to)(&selection))
}

fn startup_resolvers_matching(enabled: impl Fn(&PatchRegistration) -> bool) -> Result<Vec<&'static NamedResolver>, String> {
    crate::validate_patch_registrations()?;
    let declared: Vec<_> = inventory::iter::<DeclaredResolver>.into_iter()
        .map(|registration| &registration.resolver).collect();
    let available: Vec<_> = patternsleuth::resolvers::resolvers().collect();
    select(
        inventory::iter::<RequiredResolver>.into_iter().map(|root| root.name)
            .chain(inventory::iter::<FunctionBindingRegistration>.into_iter()
                .filter_map(|registration| registration.binding.resolver_name()))
            .chain(inventory::iter::<ValueBindingRegistration>.into_iter()
                .map(|registration| registration.binding.name()))
            .chain(inventory::iter::<PatchRegistration>.into_iter()
                .filter(|registration| enabled(registration))
                .map(|registration| registration.resolver)),
        &declared,
        &available,
    )
}

fn select<'a>(
    names: impl IntoIterator<Item = &'a str>,
    declared: &[&'static NamedResolver],
    available: &[&'static NamedResolver],
) -> Result<Vec<&'static NamedResolver>, String> {
    names.into_iter().collect::<BTreeSet<_>>().into_iter().map(|name| {
        let candidates = if declared.iter().any(|resolver| resolver.name == name) {
            declared
        } else {
            available
        };
        let mut matches = candidates.iter().copied().filter(|resolver| resolver.name == name);
        match (matches.next(), matches.next()) {
            (Some(resolver), None) => Ok(resolver),
            (None, _) => Err(format!("Required startup resolver {name} is not registered")),
            _ => Err(format!("Required startup resolver {name} is ambiguous")),
        }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unused_getter() -> &'static patternsleuth::resolvers::DynResolverFactory {
        panic!("selection must not evaluate resolvers")
    }

    static FIRST: NamedResolver = NamedResolver { name: "First", getter: unused_getter };
    static SECOND: NamedResolver = NamedResolver { name: "Second", getter: unused_getter };
    static SHADOW: NamedResolver = NamedResolver { name: "First", getter: unused_getter };

    #[test]
    fn selects_only_consumed_roots_once_in_stable_order() {
        let roots = select(["Second", "First", "First"], &[], &[&FIRST, &SECOND]).unwrap();
        assert_eq!(roots.iter().map(|root| root.name).collect::<Vec<_>>(), ["First", "Second"]);
        assert!(select([], &[], &[&FIRST]).unwrap().is_empty());
    }

    #[test]
    fn local_declarations_win_over_upstream_namesakes() {
        let roots = select(["First"], &[&FIRST], &[&SHADOW, &FIRST]).unwrap();
        assert!(std::ptr::eq(roots[0], &FIRST));
    }

    #[test]
    fn missing_and_ambiguous_roots_are_errors() {
        assert!(select(["Missing"], &[], &[&FIRST]).err().unwrap().contains("Missing"));
        assert!(select(["First"], &[], &[&FIRST, &SHADOW]).err().unwrap().contains("ambiguous"));
        assert!(select(["First"], &[&FIRST, &SHADOW], &[]).err().unwrap().contains("ambiguous"));
    }
}
