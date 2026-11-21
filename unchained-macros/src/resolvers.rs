//! Startup roots are consumers, not every resolver linked by a dependency.

use std::collections::BTreeSet;
use patternsleuth::resolvers::NamedResolver;
use crate::{FunctionBindingRegistration, ValueBindingRegistration, PatchRegistration, SelectorSnapshot};

/// Resolver identity and the selection policy used by both consumers and scans.
pub struct ResolverDescriptor {
    pub named: NamedResolver,
    pub filter: Option<crate::ResolverFilter>,
}

impl ResolverDescriptor {
    pub const fn unrestricted(named: NamedResolver) -> Self { Self { named, filter: None } }

    pub fn supports(&self, snapshot: &SelectorSnapshot) -> bool {
        self.filter.as_ref().is_none_or(|filter| snapshot.supports_filter(filter))
    }
}

/// A resolver declared by this plugin. Prefer it over an upstream namesake.
pub struct DeclaredResolver {
    pub resolver: &'static ResolverDescriptor,
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
pub fn startup_resolvers(snapshot: &SelectorSnapshot) -> Result<Vec<&'static NamedResolver>, String> {
    snapshot.validate_registered()?;
    crate::validate_patch_registrations()?;
    let declared: Vec<_> = inventory::iter::<DeclaredResolver>.into_iter()
        .map(|registration| &registration.resolver.named).collect();
    let available: Vec<_> = patternsleuth::resolvers::resolvers().collect();
    let mut roots = select(
        inventory::iter::<RequiredResolver>.into_iter().map(|root| root.name)
            .chain(inventory::iter::<ValueBindingRegistration>.into_iter()
                .map(|registration| registration.binding.name())),
        &declared,
        &available,
    )?;
    roots.retain(|resolver| snapshot.supports_resolver(resolver.name));
    roots.extend(inventory::iter::<FunctionBindingRegistration>.into_iter()
        .filter(|registration| registration.binding.supports(snapshot))
        .filter_map(|registration| match registration.binding.source() {
            crate::FunctionSource::Resolver(resolver) => Some(&resolver.named),
            crate::FunctionSource::ProviderOffset(_) => None,
        }));
    roots.extend(inventory::iter::<PatchRegistration>.into_iter()
        .filter(|registration| snapshot.supports_patch(registration) && registration.resolver.supports(snapshot))
        .map(|registration| &registration.resolver.named));
    roots.sort_by_key(|resolver| resolver.name);
    for pair in roots.windows(2) {
        if pair[0].name == pair[1].name && !std::ptr::eq(pair[0], pair[1]) {
            return Err(format!("Required startup resolver {} is ambiguous", pair[0].name));
        }
    }
    roots.dedup_by_key(|resolver| resolver.name);
    Ok(roots)
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
