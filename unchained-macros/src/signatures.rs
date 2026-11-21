//! Pattern resolver declarations and scan-mode expansion.

/// Define a named native resolver. Declarations are not startup roots until consumed.
/// See the crate's resolver reference for modes and platform dictionaries.
#[macro_export]
macro_rules! define_pattern_resolver {
    // Simple sig pattern lookup (returns start address)
    (@emit_body $name:ident, Simple, $ctx:ident, $patterns:ident) => {{
        $crate::define_pattern_resolver!(@emit_process_inline $name, |$ctx, $patterns| {
            let futures = ::patternsleuth::resolvers::futures::future::join_all(
                $patterns.iter()
                    .map(|p| $ctx.scan(::patternsleuth::scanner::Pattern::new(p).unwrap()))
            ).await;

            ::patternsleuth::resolvers::ensure_one(futures.into_iter().flatten())
        })
    }};

    (@emit_body $name:ident, First, $ctx:ident, $patterns:ident) => {{
        $crate::define_pattern_resolver!(@emit_process_inline $name, |$ctx, $patterns| {
            let futures = ::patternsleuth::resolvers::futures::future::join_all(
                $patterns.iter()
                    .map(|p| $ctx.scan(::patternsleuth::scanner::Pattern::new(p).unwrap()))
            ).await;

            futures
                .into_iter()
                .flatten()
                .next()
                .ok_or_else(||
                    ::patternsleuth::resolvers::ResolveError::new_msg(format!(
                        "Failed to find match for {name} with one of the patterns: {patterns:?}",
                        name = stringify!($name), patterns = $patterns
                    ))
            )
        })
    }};

    // Scan for a function call, extract 4 bytes and return address of the called function
    // e.g. "E8 | ?? ?? ?? ?? 4C 39 ?8 74 3?" returns address of the function after |
    (@emit_body $name:ident, Call, $ctx:ident, $patterns:ident) => {{
        use patternsleuth::MemoryTrait;
        $crate::define_pattern_resolver!(@emit_process_inline $name, |$ctx, $patterns| {
            let res = ::patternsleuth::resolvers::futures::future::join_all($patterns.iter().map(|p| $ctx.scan(patternsleuth::scanner::Pattern::new(p).unwrap()))).await;

            patternsleuth::resolvers::try_ensure_one(
                res.iter()
                    .flatten()
                    .map(|a| -> patternsleuth::resolvers::Result<u64> { Ok($ctx.image().memory.rip4(*a)?) }))
        })
    }};

    (@emit_body $name:ident, XrefLast, $ctx:ident, $patterns:ident) => {{
        $crate::define_pattern_resolver!(@emit_xref $name, last, $ctx, $patterns)
    }};
    (@emit_body $name:ident, XrefFirst, $ctx:ident, $patterns:ident) => {{
        $crate::define_pattern_resolver!(@emit_xref $name, next, $ctx, $patterns)
    }};
    // Xref modes intentionally use only the first supplied pattern.
    (@emit_xref $name:ident, $select:ident, $ctx:ident, $patterns:ident) => {{
        use patternsleuth::resolvers::unreal::util;
        use patternsleuth::resolvers::ResolveError;
        $crate::define_pattern_resolver!(@emit_process_inline $name, |$ctx, $patterns| {
            let [first_pat, ..] = $patterns.as_slice() else {
                return Err(ResolveError::new_msg(format!("expected at least one pattern for {}", stringify!($name))));
            };
            let strings = $ctx.scan(first_pat.clone()).await;
            let refs = util::scan_xrefs($ctx, &strings).await;
            util::root_functions($ctx, &refs)?.into_iter().$select()
                .ok_or_else(|| ResolveError::new_msg(format!(
                    "expected at least one pattern to have matched for {}", stringify!($name))))
        })
    }};

    // Wrap code and define_pattern_resolver
    (@emit_process_inline $name:ident, |$ctx:ident, $patterns:ident| $body:block) => {{
        let result = $body.map($name);

        if let Err(msg) = &result {
            $crate::__private::log::error!("Failed to resolve {}: {:?}", stringify!($name), msg);
        }

        result
    }};
    // Internal: produce header
    (@emit_header $name:ident) => {
        #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
        #[allow(non_camel_case_types)]
        pub struct $name(pub u64);
        $crate::__private::inventory::submit! {
            $crate::DeclaredResolver {
                resolver: ::patternsleuth::resolvers::NamedResolver {
                    name: stringify!($name),
                    getter: $name::dyn_resolver,
                },
            }
        }
    };

    // Pattern lists.
    ($name:ident, $mode:ident, [ $( $pattern:expr ),+ $(,)? ]) => {
        $crate::define_pattern_resolver!(@emit_header $name);

        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |ctx| async {
            let patterns = [ $( $pattern ),+ ];

            $crate::define_pattern_resolver!(@emit_body $name, $mode, ctx, patterns)
        });
    };

    ($name:ident, [ $( $pattern:expr ),+ $(,)? ], |$ctx:ident, $patterns:ident| $body:block) => {
        $crate::define_pattern_resolver!(@emit_header $name);

        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |$ctx| async {
            let $patterns = [ $( $pattern ),+ ];

            $crate::define_pattern_resolver!(@emit_process_inline $name, |ctx, patterns| $body)
        });
    };

    // Platform dictionaries share the same active + OTHER expansion.
    (@platform_patterns $patterns:ident, { $( $platform:ident : [ $( $pattern:expr ),+ $(,)? ] ),+ $(,)? }) => {
        for platform in [$crate::current_platform(), $crate::PlatformType::OTHER] {
            let part: &[_] = match platform {
                $( $crate::PlatformType::$platform => &[ $( $pattern ),+ ], )+
                _ => &[],
            };
            $patterns.extend_from_slice(part);
        }
    };

    ($name:ident, $mode:ident, { $( $platform:ident : [ $( $pattern:expr ),+ $(,)? ] ),+ $(,)? }) => {
        $crate::define_pattern_resolver!(@emit_header $name);

        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |ctx| async {
            let mut patterns: Vec<_> = Vec::new();
            $crate::define_pattern_resolver!(@platform_patterns patterns, {
                $( $platform: [ $( $pattern ),+ ] ),+
            });
            $crate::define_pattern_resolver!(@emit_body $name, $mode, ctx, patterns)
        });
    };

    ($name:ident, { $( $platform:ident : [ $( $pattern:expr ),+ $(,)? ] ),+ $(,)? }, |$ctx:ident, $patterns:ident| $body:block ) => {
        $crate::define_pattern_resolver!(@emit_header $name);

        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |$ctx| async {
            let mut $patterns: Vec<_> = Vec::new();
            $crate::define_pattern_resolver!(@platform_patterns $patterns, {
                $( $platform: [ $( $pattern ),+ ] ),+
            });
            $crate::define_pattern_resolver!(@emit_process_inline $name, |ctx, patterns| $body)
        });
    };

    // Default mode.
    ($name:ident, [ $( $pattern:expr ),+ $(,)? ]) => {
        $crate::define_pattern_resolver![$name, Simple,
            [ $( $pattern ),+ ]
        ];
    };

    ($name:ident, { $( $platform:ident : [ $( $pattern:expr ),+ $(,)? ] ),+ $(,)? }) => {
        $crate::define_pattern_resolver!($name, Simple, {
            $( $platform : [ $( $pattern ),+ ] ),+
        });
    };
}

/// Restrict a resolver and its hooks/patches to one platform.
/// Accepts the same modes and inline bodies as `define_pattern_resolver!`.
#[macro_export]
macro_rules! define_single_platform_pattern_resolver {
    ($platform:ident, $name:ident, $($resolver:tt)+) => {
        $crate::__private::inventory::submit! {
            $crate::ResolverPlatform {
                name: stringify!($name),
                platform: $crate::PlatformType::$platform,
            }
        }
        $crate::define_pattern_resolver!($name, $($resolver)+);
    };
}
