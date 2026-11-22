//! Scan algorithms and generated resolver metadata.

#[doc(hidden)]
#[macro_export]
macro_rules! __pattern_resolver {
    (@emit_body $name:ident, Simple, $ctx:ident, $patterns:ident) => {{
        $crate::__pattern_resolver!(@emit_process_inline $name, {
            let futures = ::patternsleuth::resolvers::futures::future::join_all(
                $patterns.iter().map(|pattern| $ctx.scan(::patternsleuth::scanner::Pattern::new(pattern).unwrap()))
            ).await;
            ::patternsleuth::resolvers::ensure_one(futures.into_iter().flatten())
        })
    }};
    (@emit_body $name:ident, First, $ctx:ident, $patterns:ident) => {{
        $crate::__pattern_resolver!(@emit_process_inline $name, {
            let futures = ::patternsleuth::resolvers::futures::future::join_all(
                $patterns.iter().map(|pattern| $ctx.scan(::patternsleuth::scanner::Pattern::new(pattern).unwrap()))
            ).await;
            futures.into_iter().flatten().next().ok_or_else(||
                ::patternsleuth::resolvers::ResolveError::new_msg(format!(
                    "Failed to find match for {name} with one of the patterns: {patterns:?}",
                    name = stringify!($name), patterns = $patterns,
                )))
        })
    }};
    (@emit_body $name:ident, Call, $ctx:ident, $patterns:ident) => {{
        use ::patternsleuth::MemoryTrait;
        $crate::__pattern_resolver!(@emit_process_inline $name, {
            let matches = ::patternsleuth::resolvers::futures::future::join_all(
                $patterns.iter().map(|pattern| $ctx.scan(::patternsleuth::scanner::Pattern::new(pattern).unwrap()))
            ).await;
            ::patternsleuth::resolvers::try_ensure_one(matches.iter().flatten().map(|address|
                -> ::patternsleuth::resolvers::Result<u64> { Ok($ctx.image().memory.rip4(*address)?) }))
        })
    }};
    (@emit_body $name:ident, XrefFirst, $ctx:ident, $patterns:ident) => {{
        $crate::__pattern_resolver!(@xref $name, next, $ctx, $patterns)
    }};
    (@emit_body $name:ident, XrefLast, $ctx:ident, $patterns:ident) => {{
        $crate::__pattern_resolver!(@xref $name, last, $ctx, $patterns)
    }};
    // Xref modes intentionally use only the first supplied pattern.
    (@xref $name:ident, $select:ident, $ctx:ident, $patterns:ident) => {{
        use ::patternsleuth::resolvers::{unreal::util, ResolveError};
        $crate::__pattern_resolver!(@emit_process_inline $name, {
            let [first, ..] = $patterns.as_slice() else {
                return Err(ResolveError::new_msg(format!("expected at least one pattern for {}", stringify!($name))));
            };
            let strings = $ctx.scan(first.clone()).await;
            let refs = util::scan_xrefs($ctx, &strings).await;
            util::root_functions($ctx, &refs)?.into_iter().$select().ok_or_else(||
                ResolveError::new_msg(format!("expected at least one pattern to have matched for {}", stringify!($name))))
        })
    }};
    (@emit_process_inline $name:ident, $body:block) => {{
        let result = $body.map($name);
        if let Err(error) = &result {
            $crate::__private::log::error!("Failed to resolve {}: {:?}", stringify!($name), error);
        }
        result
    }};
    (@emit_type $name:ident, $unrestricted:expr, $filter:expr) => {
        #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
        #[allow(non_camel_case_types)]
        pub struct $name(pub u64);
        impl $crate::ResolverDeclaration for $name {
            type Backend = $crate::PatternsleuthResolver;
            const NAME: &'static str = stringify!($name);
            const UNRESTRICTED: bool = $unrestricted;
            const DESCRIPTOR: &'static $crate::ResolverDescriptor = {
                static DESCRIPTOR: $crate::ResolverDescriptor = $crate::ResolverDescriptor {
                    name: stringify!($name),
                    implementation: $crate::PatternsleuthResolver::named(::patternsleuth::resolvers::NamedResolver {
                        name: stringify!($name), getter: $name::dyn_resolver,
                    }),
                    filter: $filter,
                };
                &DESCRIPTOR
            };
        }
        $crate::__private::inventory::submit! {
            $crate::__private::ErasedResolverRegistration::new(<$name as $crate::ResolverDeclaration>::DESCRIPTOR)
        }
    };
    (@emit_header $name:ident) => {
        $crate::__pattern_resolver!(@emit_type $name, true, ::core::option::Option::None);
        impl<E: $crate::SelectionKey> $crate::ResolverSelection<E> for $name {
            const SELECTIONS: &'static [(E, bool)] = &[];
        }
    };
    ($name:ident, $mode:ident, [$($pattern:expr),+ $(,)?]) => {
        $crate::__pattern_resolver!(@emit_header $name);
        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |ctx| async {
            let patterns = [$($pattern),+];
            $crate::__pattern_resolver!(@emit_body $name, $mode, ctx, patterns)
        });
    };
    ($name:ident, [$($pattern:expr),+ $(,)?], |$ctx:ident, $patterns:ident| $body:block) => {
        $crate::__pattern_resolver!(@emit_header $name);
        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |$ctx| async {
            let $patterns = [$($pattern),+];
            $crate::__pattern_resolver!(@emit_process_inline $name, $body)
        });
    };
    (@enabled None) => { false };
    (@enabled [$($pattern:expr),* $(,)?]) => { true };
    (@patterns None) => { &[] };
    (@patterns [$($pattern:expr),* $(,)?]) => { &[$($pattern),*] };
    (@new_patterns $mode:ident; None, $($rest:tt)*) => {
        $crate::__pattern_resolver!(@new_patterns $mode; $($rest)*)
    };
    (@new_patterns $mode:ident; [$($pattern:expr),+ $(,)?], $($rest:tt)*) => { Vec::new() };
    (@new_patterns XrefFirst;) => { Vec::<::patternsleuth::scanner::Pattern>::new() };
    (@new_patterns XrefLast;) => { Vec::<::patternsleuth::scanner::Pattern>::new() };
    (@new_patterns $mode:ident;) => { Vec::<&str>::new() };
    (@restriction $name:ident, $enum:ty, {$($key:path: $entry:tt),+}) => {
        const _: () = {
            let keys = [$($key),+];
            keys[0].__unchained_validate_selection(&keys);
        };
        impl $crate::ResolverSelection<$enum> for $name {
            const SELECTIONS: &'static [($enum, bool)] = &[
                $(($key, $crate::__pattern_resolver!(@enabled $entry))),+
            ];
        }
        $crate::__private::inventory::submit! {
            $crate::__private::SelectorProvider::of::<$enum>()
        }
    };
    (@selected_patterns $name:ident, $patterns:ident, $ctx:ident, $enum:ty, {$($key:path: $entry:tt),+}) => {
        let snapshot = $crate::__private::context_snapshot($ctx).await?;
        if !<$name as $crate::ResolverDeclaration>::DESCRIPTOR.supports(&snapshot) {
            return Err(::patternsleuth::resolvers::ResolveError::new_msg(format!(
                "Resolver {} is disabled for the selected key", stringify!($name))));
        }
        for key in snapshot.chain::<$enum>().map_err(::patternsleuth::resolvers::ResolveError::new_msg)? {
            let part: ::core::option::Option<&[_]> = match key {
                $($key => ::core::option::Option::Some($crate::__pattern_resolver!(@patterns $entry)),)+
                #[allow(unreachable_patterns)]
                _ => ::core::option::Option::None,
            };
            if let ::core::option::Option::Some(part) = part {
                $patterns.extend_from_slice(part);
                break;
            }
        }
    };
    ($name:ident, $mode:ident, $enum:ty, {$($key:path: $entry:tt),+ $(,)?}) => {
        $crate::__pattern_resolver!(@emit_type $name, true $(&& $crate::__pattern_resolver!(@enabled $entry))+, $crate::__pattern_resolver!(@filter $name, $enum));
        $crate::__pattern_resolver!(@restriction $name, $enum, {$($key: $entry),+});
        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |ctx| async {
            let mut patterns = $crate::__pattern_resolver!(@new_patterns $mode; $($entry),+,);
            $crate::__pattern_resolver!(@selected_patterns $name, patterns, ctx, $enum, {$($key: $entry),+});
            $crate::__pattern_resolver!(@emit_body $name, $mode, ctx, patterns)
        });
    };
    ($name:ident, $enum:ty, {$($key:path: $entry:tt),+ $(,)?}, |$ctx:ident, $patterns:ident| $body:block) => {
        $crate::__pattern_resolver!(@emit_type $name, true $(&& $crate::__pattern_resolver!(@enabled $entry))+, $crate::__pattern_resolver!(@filter $name, $enum));
        $crate::__pattern_resolver!(@restriction $name, $enum, {$($key: $entry),+});
        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |$ctx| async {
            let mut $patterns = $crate::__pattern_resolver!(@new_patterns Inline; $($entry),+,);
            $crate::__pattern_resolver!(@selected_patterns $name, $patterns, $ctx, $enum, {$($key: $entry),+});
            $crate::__pattern_resolver!(@emit_process_inline $name, $body)
        });
    };
    (@filter $name:ident, $enum:ty) => {
        ::core::option::Option::Some($crate::ResolverFilter {
            selector: ::std::any::TypeId::of::<$enum>,
            lookup: |key| $crate::__private::dictionary_lookup(
                key, <$name as $crate::ResolverSelection<$enum>>::SELECTIONS,
            ),
        })
    };
}
