
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


#[macro_export]
macro_rules! define_pocess {

    // Simple sig pattern lookup (returns start address)
    (@emit_body $name:ident, Simple, $ctx:ident, $patterns:ident) => {{
        define_pocess!(@emit_process_inline $name, |$ctx, $patterns| {
            let futures = ::patternsleuth::resolvers::futures::future::join_all(
                $patterns.iter()
                    .map(|p| $ctx.scan(::patternsleuth::scanner::Pattern::new(p).unwrap()))
            ).await;
    
            ::patternsleuth::resolvers::ensure_one(futures.into_iter().flatten())?
        })
    }};

    (@emit_body $name:ident, First, $ctx:ident, $patterns:ident) => {{
        define_pocess!(@emit_process_inline $name, |$ctx, $patterns| {
            let futures = ::patternsleuth::resolvers::futures::future::join_all(
                $patterns.iter()
                    .map(|p| $ctx.scan(::patternsleuth::scanner::Pattern::new(p).unwrap()))
            ).await;
    
            futures.into_iter().flatten().collect::<Vec<usize>>()[0]
        })
    }};

    // Scan for a function call, extract 4 bytes and return address of the called function
    // e.g. "E8 | ?? ?? ?? ?? 4C 39 ?8 74 3?" returns address of the function after |
    (@emit_body $name:ident, Call, $ctx:ident, $patterns:ident) => {{
        use patternsleuth::MemoryTrait;
        define_pocess!(@emit_process_inline $name, |$ctx, $patterns| {
            let res = futures::future::join_all($patterns.iter().map(|p| $ctx.scan(patternsleuth::scanner::Pattern::new(p).unwrap()))).await;
            
            patternsleuth::resolvers::try_ensure_one(
                res.iter()
                    .flatten()
                    .map(|a| -> patternsleuth::resolvers::Result<usize> { Ok($ctx.image().memory.rip4(*a)?) }))?
        })
    }};

    // Scan for Xrefs, return last
    // FIXME: expects max. 2 results
    // FIXME: handles only one pattern
    (@emit_body $name:ident, XrefLast, $ctx:ident, $patterns:ident) => {{
        use patternsleuth::resolvers::unreal::util;
        use patternsleuth::resolvers::ensure_one;
        define_pocess!(@emit_process_inline $name, |$ctx, $patterns| {
            // let strings = futures::future::join_all(patterns.iter().map(|p| ctx.scan(p.clone()))).await;
            let strings = $ctx.scan($patterns.first().unwrap().clone()).await;
            let refs = util::scan_xrefs($ctx, &strings).await;
            let mut fns = util::root_functions($ctx, &refs)?;
            if fns.len() == 2 {
                fns[0] = fns[1]; // FIXME: deque? last?
                fns.pop();
            }
            ensure_one(fns)?
        })
    }};

    // Wrap code and define_pattern_resolver
    (@emit_process_inline $name:ident, |$ctx:ident, $patterns:ident| $body:block) => {{
        let result = $body;
        Ok($name(result))
    }};
}

#[macro_export]
macro_rules! define_pattern_resolver {

    // Internal: produce header
    (@emit_header $name:ident) => {
        #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
        #[allow(non_camel_case_types)]
        pub struct $name(pub usize);
    };

    // Called with Mode (Process) specified
    //  (NAME, MODE, LIST) 
    ($name:ident, $mode:ident, [ $( $pattern:expr ),+ $(,)? ]) => {
        define_pattern_resolver!(@emit_header $name);

        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |ctx| async {
            let patterns = [ $( $pattern ),+ ];

            define_pocess!(@emit_body $name, $mode, ctx, patterns)
        });
    };

    // Called with a body block
    // (NAME, LIST, |ctx, patterns| { CODE } )
    ($name:ident, [ $( $pattern:expr ),+ $(,)? ], |$ctx:ident, $patterns:ident| $body:block) => {
        define_pattern_resolver!(@emit_header $name);

        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |$ctx| async {
            let $patterns = [ $( $pattern ),+ ];

            define_pocess!(@emit_process_inline $name, |ctx, patterns| $body)
        });
    };

    // Called with Mode (Process) specified
    //  (NAME, MODE, DICT) 
    ($name:ident, $mode:ident, { $( $platform:ident : [ $( $pattern:expr ),+ $(,)? ] ),+ $(,)? }) => {
        define_pattern_resolver!(@emit_header $name);

        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |ctx| async {
            let platform = super::current_platform();
            let mut patterns: Vec<&str> = Vec::new();
            for var in [platform, super::PlatformType::OTHER] {
                // println!("Testing {}", stringify!($name));                
                let pattern_part: &[&str] = match var {
                    $(
                        super::PlatformType::$platform => &[ $( $pattern ),+ ],
                    )+
                    _ => &[],
                };                
                // println!("{}: {:?}", var, pattern_part);
                patterns.extend_from_slice(pattern_part);
            }            
            // println!("combined {:?}", patterns);            
            define_pocess!(@emit_body $name, $mode, ctx, patterns)
        });
    };

    // Called with a body block
    //  (NAME, MODE, DICT) 
    ($name:ident, { $( $platform:ident : [ $( $pattern:expr ),+ $(,)? ] ),+ $(,)? }, |$ctx:ident, $patterns:ident| $body:block ) => {
        define_pattern_resolver!(@emit_header $name);
        
        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |$ctx| async {
            let platform = super::current_platform();
            let mut $patterns: Vec<&str> = Vec::new();
            for var in [platform, super::PlatformType::OTHER] {
                // println!("Testing {}", stringify!($name));                
                let pattern_part: &[&str] = match var {
                    $(
                        super::PlatformType::$platform => &[ $( $pattern ),+ ],
                    )+
                    _ => &[],
                };                
                // println!("{}: {:?}", var, pattern_part);
                $patterns.extend_from_slice(pattern_part);
            }            
            // println!("combined {:?}", patterns);            

            define_pocess!(@emit_process_inline $name, |ctx, patterns| $body)
        });
    };

    // Fallbacks for cases with no mode specified (assuming Simple)
    
    //  (NAME, LIST) 
    ($name:ident, [ $( $pattern:expr ),+ $(,)? ]) => {
        define_pattern_resolver![$name, Simple,
            [ $( $pattern ),+ ]
        ];
    };

    //  (NAME, DICT) 
    ($name:ident, { $( $platform:ident : [ $( $pattern:expr ),+ $(,)? ] ),+ $(,)? }) => {
        define_pattern_resolver!($name, Simple, {
            $( $platform : [ $( $pattern ),+ ] ),+
        });
    };
}

#[macro_export]
macro_rules! define_process {
    ($src:ident, $name:ident, $body:block) => {
        async fn $name(
            ctx: &patternsleuth::resolvers::AsyncContext<'_>,
            patterns: Vec<patternsleuth::scanner::Pattern>
        ) -> patternsleuth::resolvers::Result<$src> {
            let result: Vec<usize> = Vec::new();
            $body
            Ok($src(ensure_one(result)?))
        }
    };
}