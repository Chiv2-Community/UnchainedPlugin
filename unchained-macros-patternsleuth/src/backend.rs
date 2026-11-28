use patternsleuth::resolvers::NamedResolver;
use std::{
    collections::{BTreeSet, HashMap},
    num::NonZeroUsize,
};

use unchained_macros_core::*;

pub struct ScanContext {
    pub base: usize,
    pub offsets: HashMap<String, u64>,
}

impl ScanContext {
    pub fn new(base: usize, offsets: &HashMap<String, u64>) -> Self {
        Self {
            base,
            offsets: offsets.clone(),
        }
    }
}

pub struct PatternsleuthResolver {
    pub key: &'static str,
    pub named: Option<NamedResolver>,
}

impl PatternsleuthResolver {
    pub const fn named(named: NamedResolver) -> Self {
        Self {
            key: named.name,
            named: Some(named),
        }
    }

    pub const fn offset(key: &'static str) -> Self {
        Self { key, named: None }
    }
}

impl AddressResolver for PatternsleuthResolver {
    type Context = ScanContext;

    unsafe fn resolve(&self, context: &ScanContext) -> Result<NonZeroUsize, String> {
        context
            .offsets
            .get(self.key)
            .and_then(|offset| usize::try_from(*offset).ok())
            .and_then(|offset| context.base.checked_add(offset))
            .filter(|_| context.base != 0)
            .and_then(NonZeroUsize::new)
            .ok_or_else(|| format!("No valid address for resolver {}", self.key))
    }
}

pub struct RequiredResolver {
    pub name: &'static str,
}

pub struct OffsetProvider {
    pub name: &'static str,
    pub map: fn() -> HashMap<String, u64>,
}

inventory::collect!(RequiredResolver);

inventory::collect!(OffsetProvider);

/// Declare a typed resolver for an RVA supplied by an offset-map provider.
#[macro_export]
macro_rules! define_offset_resolver {
    ($name:ident) => {
        #[allow(non_camel_case_types)]

        pub struct $name;

        impl ::unchained_macros_core::ResolverDeclaration for $name {
            type Backend = $crate::PatternsleuthResolver;

            const NAME: &'static str = stringify!($name);

            const UNRESTRICTED: bool = true;

            const DESCRIPTOR: &'static ::unchained_macros_core::Resolver<Self::Backend> = {
                static DESCRIPTOR: ::unchained_macros_core::Resolver<$crate::PatternsleuthResolver> =
                    ::unchained_macros_core::Resolver {
                        name: stringify!($name),
                        implementation: $crate::PatternsleuthResolver::offset(stringify!($name)),
                        filter: None,
                    };

                &DESCRIPTOR
            };
        }

        impl<E: ::unchained_macros_core::SelectionKey> ::unchained_macros_core::ResolverSelection<E> for $name {
            const SELECTIONS: &'static [(E, bool)] = &[];
        }

        $crate::__private::inventory::submit! {
            <$name as ::unchained_macros_core::ResolverDeclaration>::DESCRIPTOR
                as &dyn $crate::__private::ResolverInfo
        }
    };
}

#[macro_export]
macro_rules! require_resolver {
    ($name:ident) => {
        $crate::__private::inventory::submit! { $crate::RequiredResolver { name: stringify!($name) } }
    };
}

pub fn startup_resolvers(snapshot: &SelectorSnapshot) -> Result<Vec<&'static NamedResolver>, String> {
    snapshot.validate_registered()?;

    validate_patch_registrations()?;

    let declared: Vec<_> = resolver_inventory_for::<PatternsleuthResolver>()
        .filter_map(|resolver| resolver.implementation.named.as_ref())
        .collect();

    let available: Vec<_> = patternsleuth::resolvers::resolvers().collect();

    let names = inventory::iter::<RequiredResolver>
        .into_iter()
        .map(|root| root.name)
        .chain(
            inventory::iter::<&'static ValueBinding>
                .into_iter()
                .map(|entry| entry.name()),
        );

    let mut roots = select(names, &declared, &available)?;

    roots.retain(|resolver| snapshot.supports_resolver_for::<PatternsleuthResolver>(resolver.name));

    roots.extend(
        binding_inventory_for::<PatternsleuthResolver>()
            .filter(|entry| entry.supports(snapshot))
            .filter_map(|entry| entry.resolver.implementation.named.as_ref()),
    );

    roots.extend(
        patch_inventory_for::<PatternsleuthResolver>()
            .filter(|entry| snapshot.supports_patch(*entry))
            .filter_map(|entry| entry.resolver.implementation.named.as_ref()),
    );

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
    names
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|name| {
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
        })
        .collect()
}

#[cfg(test)]
mod tests {

    use super::*;

    fn unused_getter() -> &'static patternsleuth::resolvers::DynResolverFactory {
        panic!("selection must not evaluate resolvers")
    }

    static FIRST: NamedResolver = NamedResolver {
        name: "First",
        getter: unused_getter,
    };

    static SECOND: NamedResolver = NamedResolver {
        name: "Second",
        getter: unused_getter,
    };

    static SHADOW: NamedResolver = NamedResolver {
        name: "First",
        getter: unused_getter,
    };

    #[test]

    fn selects_only_consumed_roots_once_in_stable_order() {
        let roots = select(["Second", "First", "First"], &[], &[&FIRST, &SECOND]).unwrap();

        assert_eq!(
            roots.iter().map(|root| root.name).collect::<Vec<_>>(),
            ["First", "Second"]
        );

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

        assert!(select(["First"], &[], &[&FIRST, &SHADOW])
            .err()
            .unwrap()
            .contains("ambiguous"));

        assert!(select(["First"], &[&FIRST, &SHADOW], &[])
            .err()
            .unwrap()
            .contains("ambiguous"));
    }
}
