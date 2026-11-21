//! Pattern resolver declarations and scan-mode expansion.

/// Define a named native resolver. Declarations are not startup roots until consumed.
/// See the crate's resolver reference for modes and selection dictionaries.
///
/// Dictionaries accept fieldless enum keys, never integer discriminants:
/// ```compile_fail
/// # fn main() {}
/// unchained_macros::define_pattern_resolver!(Invalid, select [0u8], {
///     0: ["AA BB"],
/// });
/// ```
///
/// Required selections must be explicit, including disabled selections:
/// ```compile_fail
/// # fn main() {}
/// #[unchained_macros::resolver_selection_key(require = [Read, Write])]
/// enum Mode { Read, Write }
/// unchained_macros::define_pattern_resolver!(Invalid, select [Mode::Read], {
///     Mode::Read: ["AA BB"],
/// });
/// ```
///
/// A `None` entry still participates in duplicate-key checks:
/// ```compile_fail
/// # fn main() {}
/// #[unchained_macros::resolver_selection_key]
/// enum Mode { Read }
/// unchained_macros::define_pattern_resolver!(Invalid, select [Mode::Read], {
///     Mode::Read: None,
///     Mode::Read: ["AA BB"],
/// });
/// ```
///
/// Unknown variants and mixed enum types are rejected:
/// ```compile_fail
/// # fn main() {}
/// #[unchained_macros::resolver_selection_key]
/// enum Mode { Read }
/// unchained_macros::define_pattern_resolver!(Invalid, select [Mode::Read], {
///     Mode::Missing: ["AA BB"],
/// });
/// ```
/// ```compile_fail
/// # fn main() {}
/// #[unchained_macros::resolver_selection_key]
/// enum Mode { Read }
/// #[unchained_macros::resolver_selection_key]
/// enum Other { Read }
/// unchained_macros::define_pattern_resolver!(Invalid, select [Mode::Read], {
///     Other::Read: ["AA BB"],
/// });
/// ```
///
/// Duplicate dictionary keys are rejected:
/// ```compile_fail
/// # fn main() {}
/// #[unchained_macros::resolver_selection_key]
/// enum Mode { Read }
/// unchained_macros::define_pattern_resolver!(Invalid, select [Mode::Read], {
///     Mode::Read: ["AA BB"],
///     Mode::Read: ["CC DD"],
/// });
/// ```
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

    (@enabled None) => { false };
    (@enabled [$($pattern:expr),* $(,)?]) => { true };
    (@patterns None) => { &[] };
    (@patterns [$($pattern:expr),* $(,)?]) => { &[$($pattern),*] };
    // Empty dictionaries still need a concrete type, even when disabled.
    (@new_patterns $mode:ident; None, $($rest:tt)*) => {
        $crate::define_pattern_resolver!(@new_patterns $mode; $($rest)*)
    };
    (@new_patterns $mode:ident; [], $($rest:tt)*) => {
        $crate::define_pattern_resolver!(@new_patterns $mode; $($rest)*)
    };
    (@new_patterns $mode:ident; [$($pattern:expr),+ $(,)?], $($rest:tt)*) => { Vec::new() };
    (@new_patterns XrefFirst;) => { Vec::<::patternsleuth::scanner::Pattern>::new() };
    (@new_patterns XrefLast;) => { Vec::<::patternsleuth::scanner::Pattern>::new() };
    (@new_patterns $mode:ident;) => { Vec::<&str>::new() };

    (@restriction $name:ident, { $($key:path: $entry:tt),+ }) => {
        const _: () = {
            let keys = [$($key),+];
            keys[0].__unchained_validate_selection(&keys);
        };
        $crate::__private::inventory::submit! {
            $crate::ResolverRestriction {
                name: stringify!($name),
                matches: |key| $crate::__private::dictionary_allows(key, &[
                    $(($key, $crate::define_pattern_resolver!(@enabled $entry))),+
                ]),
            }
        }
    };

    // The first key controls applicability; later keys only contribute patterns.
    (@selected_patterns $name:ident, $patterns:ident, [$($selection:expr),+], { $( $key:path : $entry:tt ),+ $(,)? }) => {
        let selections = [$($crate::__private::selection_key($selection)),+];
        if !$crate::supports_selection(stringify!($name), selections[0]) {
            return Err(::patternsleuth::resolvers::ResolveError::new_msg(format!(
                "Resolver {} is disabled for the selected key", stringify!($name))));
        }
        for key in selections {
            let part: &[_] = match key {
                $( $key => $crate::define_pattern_resolver!(@patterns $entry), )+
                #[allow(unreachable_patterns)]
                _ => &[],
            };
            $patterns.extend_from_slice(part);
        }
    };

    ($name:ident, $mode:ident, select [$($selection:expr),+ $(,)?], { $( $key:path : $entry:tt ),+ $(,)? }) => {
        $crate::define_pattern_resolver!(@emit_header $name);
        $crate::define_pattern_resolver!(@restriction $name, { $($key: $entry),+ });

        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |ctx| async {
            let mut patterns = $crate::define_pattern_resolver!(@new_patterns $mode; $($entry),+,);
            $crate::define_pattern_resolver!(@selected_patterns $name, patterns, [$($selection),+], {
                $( $key: $entry ),+
            });
            $crate::define_pattern_resolver!(@emit_body $name, $mode, ctx, patterns)
        });
    };

    ($name:ident, select [$($selection:expr),+ $(,)?], { $( $key:path : $entry:tt ),+ $(,)? }, |$ctx:ident, $patterns:ident| $body:block ) => {
        $crate::define_pattern_resolver!(@emit_header $name);
        $crate::define_pattern_resolver!(@restriction $name, { $($key: $entry),+ });

        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |$ctx| async {
            let mut $patterns = $crate::define_pattern_resolver!(@new_patterns Inline; $($entry),+,);
            $crate::define_pattern_resolver!(@selected_patterns $name, $patterns, [$($selection),+], {
                $( $key: $entry ),+
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

    ($name:ident, select [$($selection:expr),+ $(,)?], { $( $key:path : $entry:tt ),+ $(,)? }) => {
        $crate::define_pattern_resolver!($name, Simple, select [$($selection),+], {
            $( $key : $entry ),+
        });
    };
}
