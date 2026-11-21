//! Startup roots are consumers, not every resolver linked by a dependency.

use std::collections::BTreeSet;
use patternsleuth::resolvers::NamedResolver;

/// A resolver declared by this plugin. Prefer it over an upstream namesake.
pub struct DeclaredResolver {
    pub resolver: NamedResolver,
}

/// A cache key required by a compiled binding, patch, or explicit scan consumer.
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
    let declared: Vec<_> = inventory::iter::<DeclaredResolver>.into_iter()
        .map(|registration| &registration.resolver).collect();
    let available: Vec<_> = patternsleuth::resolvers::resolvers().collect();
    select(
        inventory::iter::<RequiredResolver>.into_iter().map(|root| root.name),
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
        let local: Vec<_> = declared.iter().copied().filter(|resolver| resolver.name == name).collect();
        let candidates: Vec<_> = if local.is_empty() {
            available.iter().copied().filter(|resolver| resolver.name == name).collect()
        } else {
            local
        };
        match candidates.as_slice() {
            [resolver] => Ok(*resolver),
            [] => Err(format!("Required startup resolver {name} is not registered")),
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
