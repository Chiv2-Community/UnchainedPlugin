
// Testing SomeTestFkt
// EGS: ["40 53 FF", "40 53 48"]
// Testing SomeTestFkt
// OTHER: ["11 53 B8", "22 53 FF", "40 53 4F", "40 53 A8"]
// combined ["40 53 FF", "40 53 48", "11 53 B8", "22 53 FF", "40 53 4F", "40 53 A8"]
// define_pattern_resolver![SomeTestFkt, {
//     OTHER : ["11 53 B8", "22 53 FF", "40 53 4F", "40 53 A8"],
//     EGS : ["40 53 FF", "40 53 48"],
//     XBOX : ["40 53 48", "40 53 48"],
//     STEAM : ["40 53 48", "40 53 48", "40 53 48", "40 53 48"],
// }];

// #[macro_use]
// extern crate paste; // concat strings

#[macro_export]
macro_rules! define_pocess {
    // Simple sig pattern lookup (returns start address)
    (@emit_body $name:ident, Simple, $ctx:ident, $patterns:ident) => {{
        $crate::define_pocess!(@emit_process_inline $name, |$ctx, $patterns| {
            let futures = ::patternsleuth::resolvers::futures::future::join_all(
                $patterns.iter()
                    .map(|p| $ctx.scan(::patternsleuth::scanner::Pattern::new(p).unwrap()))
            ).await;

            ::patternsleuth::resolvers::ensure_one(futures.into_iter().flatten())
        })
    }};

    (@emit_body $name:ident, First, $ctx:ident, $patterns:ident) => {{
        $crate::define_pocess!(@emit_process_inline $name, |$ctx, $patterns| {
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
        $crate::define_pocess!(@emit_process_inline $name, |$ctx, $patterns| {
            let res = ::patternsleuth::resolvers::futures::future::join_all($patterns.iter().map(|p| $ctx.scan(patternsleuth::scanner::Pattern::new(p).unwrap()))).await;

            patternsleuth::resolvers::try_ensure_one(
                res.iter()
                    .flatten()
                    .map(|a| -> patternsleuth::resolvers::Result<u64> { Ok($ctx.image().memory.rip4(*a)?) }))
        })
    }};

    // Scan for Xrefs, return last
    // FIXME: handles only one pattern
    (@emit_body $name:ident, XrefLast, $ctx:ident, $patterns:ident) => {{
        use patternsleuth::resolvers::unreal::util;
        use patternsleuth::resolvers::ensure_one;
        use patternsleuth::resolvers::ResolveError;
        $crate::define_pocess!(@emit_process_inline $name, |$ctx, $patterns| {
            // let strings = futures::future::join_all(patterns.iter().map(|p| ctx.scan(p.clone()))).await;
            let [first_pat, ..] = $patterns.as_slice() else {
                return Err(ResolveError::new_msg(format!("expected at least one pattern for {}", stringify!($name))));
            };
            let strings = $ctx.scan(first_pat.clone()).await;
            let refs = util::scan_xrefs($ctx, &strings).await;
            let mut fns = util::root_functions($ctx, &refs)?;
            let [.., last_fn] = fns.as_slice() else { return Err(ResolveError::new_msg(format!("expected at least one pattern to have matched for {}", stringify!($name)))) };
            let last_fn = last_fn.clone();
            fns.clear();
            fns.push(last_fn);
            ensure_one(fns)
        })
    }};
    (@emit_body $name:ident, XrefFirst, $ctx:ident, $patterns:ident) => {{
        use patternsleuth::resolvers::unreal::util;
        use patternsleuth::resolvers::ensure_one;
        use patternsleuth::resolvers::ResolveError;
        $crate::define_pocess!(@emit_process_inline $name, |$ctx, $patterns| {
            // let strings = futures::future::join_all(patterns.iter().map(|p| ctx.scan(p.clone()))).await;
            let [first_pat, ..] = $patterns.as_slice() else {
                return Err(ResolveError::new_msg(format!("expected at least one pattern for {}", stringify!($name))));
            };
            let strings = $ctx.scan(first_pat.clone()).await;
            let refs = util::scan_xrefs($ctx, &strings).await;
            let mut fns = util::root_functions($ctx, &refs)?;
            let [first_fn, ..] = fns.as_slice() else { return Err(ResolveError::new_msg(format!("expected at least one pattern to have matched for {}", stringify!($name)))) };
            let first_fn = first_fn.clone();
            fns.clear();
            fns.push(first_fn);
            ensure_one(fns)
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
}

/// Define a resolver restricted to one platform, including its hooks and patches.
/// Accepts the same pattern lists, modes, and inline bodies as `define_pattern_resolver!`.
///
/// ```rust,ignore
/// define_single_platform_pattern_resolver!(STEAM, SteamOnlyCheck, ["48 8B ?? ??"]);
/// CREATE_PATCH!(SteamOnlyCheck, NOP, 2);
/// ```
#[macro_export]
macro_rules! define_single_platform_pattern_resolver {
    ($platform:ident, $name:ident, $($resolver:tt)+) => {
        inventory::submit! {
            $crate::ResolverPlatform {
                name: stringify!($name),
                platform: $crate::PlatformType::$platform,
            }
        }
        $crate::define_pattern_resolver!($name, $($resolver)+);
    };
}

#[macro_export]
macro_rules! define_pattern_resolver {

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

    // Called with Mode (Process) specified
    //  (NAME, MODE, LIST)
    ($name:ident, $mode:ident, [ $( $pattern:expr ),+ $(,)? ]) => {
        $crate::define_pattern_resolver!(@emit_header $name);

        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |ctx| async {
            let patterns = [ $( $pattern ),+ ];

            $crate::define_pocess!(@emit_body $name, $mode, ctx, patterns)
        });
    };

    // Called with a body block
    // (NAME, LIST, |ctx, patterns| { CODE } )
    ($name:ident, [ $( $pattern:expr ),+ $(,)? ], |$ctx:ident, $patterns:ident| $body:block) => {
        $crate::define_pattern_resolver!(@emit_header $name);

        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |$ctx| async {
            let $patterns = [ $( $pattern ),+ ];

            $crate::define_pocess!(@emit_process_inline $name, |ctx, patterns| $body)
        });
    };

    // Called with Mode (Process) specified
    //  (NAME, MODE, DICT)
    ($name:ident, $mode:ident, { $( $platform:ident : [ $( $pattern:expr ),+ $(,)? ] ),+ $(,)? }) => {
        $crate::define_pattern_resolver!(@emit_header $name);

        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |ctx| async {
            let platform = $crate::current_platform();
            let mut patterns: Vec<_> = Vec::new();
            for var in [platform, $crate::PlatformType::OTHER] {
                // println!("Testing {}", stringify!($name));
                let pattern_part: &[_] = match var {
                    $(
                        $crate::PlatformType::$platform => &[ $( $pattern ),+ ],
                    )+
                    _ => &[],
                };
                // println!("{}: {:?}", var, pattern_part);
                patterns.extend_from_slice(pattern_part);
            }
            // println!("combined {:?}", patterns);
            $crate::define_pocess!(@emit_body $name, $mode, ctx, patterns)
        });
    };

    // Called with a body block
    //  (NAME, MODE, DICT)
    ($name:ident, { $( $platform:ident : [ $( $pattern:expr ),+ $(,)? ] ),+ $(,)? }, |$ctx:ident, $patterns:ident| $body:block ) => {
        $crate::define_pattern_resolver!(@emit_header $name);

        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |$ctx| async {
            let platform = $crate::current_platform();
            let mut $patterns: Vec<_> = Vec::new();
            for var in [platform, $crate::PlatformType::OTHER] {
                // println!("Testing {}", stringify!($name));
                let pattern_part: &[_] = match var {
                    $(
                        $crate::PlatformType::$platform => &[ $( $pattern ),+ ],
                    )+
                    _ => &[],
                };
                // println!("{}: {:?}", var, pattern_part);
                $patterns.extend_from_slice(pattern_part);
            }
            // println!("combined {:?}", patterns);

            $crate::define_pocess!(@emit_process_inline $name, |ctx, patterns| $body)
        });
    };

    // Fallbacks for cases with no mode specified (assuming Simple)

    //  (NAME, LIST)
    // define_pattern_resolver!(GetMotdTwoS, [
    //         "4C 89",
    //         "4C 89"
    //     ]);
    ($name:ident, [ $( $pattern:expr ),+ $(,)? ]) => {
        $crate::define_pattern_resolver![$name, Simple,
            [ $( $pattern ),+ ]
        ];
    };

    //  (NAME, DICT)
    // define_pattern_resolver!(GetMotdTwoS, {
    //     EGS: [
    //         "4C 89",
    //         "4C 89"
    //     ],
    //     STEAM: ["4C 89 | ?? ?? ?? ?? BE EF"]
    // });
    ($name:ident, { $( $platform:ident : [ $( $pattern:expr ),+ $(,)? ] ),+ $(,)? }) => {
        $crate::define_pattern_resolver!($name, Simple, {
            $( $platform : [ $( $pattern ),+ ] ),+
        });
    };
}
