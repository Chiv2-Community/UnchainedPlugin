
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
macro_rules! define_pattern_resolver {
    
    //  (NAME, MODE, DICT) -> emit(NAME, MODE, DICT)
    ($name:ident, $mode:ident, { $( $platform:ident : [ $( $pattern:expr ),+ $(,)? ] ),+ $(,)? }) => {
        define_pattern_resolver!(@emit_body $name, $mode, {
            $( $platform : [ $( $pattern ),+ ] ),+
        });
    };

    //  (NAME, DICT) -> macro(NAME, Simple, DICT)
    ($name:ident, { $( $platform:ident : [ $( $pattern:expr ),+ $(,)? ] ),+ $(,)? }) => {

        define_pattern_resolver!($name, Simple, {
            $( $platform : [ $( $pattern ),+ ] ),+
        });
    };

    //  emit(NAME, MODE, DICT) -> ...
    (@emit_body $name:ident, $mode:ident, { $( $platform:ident : [ $( $pattern:expr ),+ $(,)? ] ),+ $(,)? }) => {

        #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
        #[allow(non_camel_case_types)]
        pub struct $name( pub usize);

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
            
            println!("combined {:?}", patterns);

            define_pattern_resolver![@emit_body $name, $mode];

            process(ctx, patterns).await

            // Ok($name(
            //     123
            //     // ::patternsleuth::resolvers::ensure_one(futures.into_iter().flatten())?
            // ))
            // Ok($name(12345))
        });
    };
    
    //  (NAME, MODE, LIST) -> emit function body, emit(NAME, MODE)
    ($name:ident, $mode:ident, [ $( $pattern:expr ),+ $(,)? ]) => {
        
        #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
        #[allow(non_camel_case_types)]
        pub struct $name(pub usize);

        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |ctx| async {
            let patterns = [ $( $pattern ),+ ];

            define_pattern_resolver![@emit_body $name, $mode];

            process(ctx, patterns.to_vec()).await
        });
    };

    //  (NAME, LIST) -> (NAME, Simple, LIST)
    ($name:ident, [ $( $pattern:expr ),+ $(,)? ]) => {
        define_pattern_resolver![$name, Simple,
            [ $( $pattern ),+ ]
        ];
        
    };

    // Process implementation

    (@emit_body $name:ident, Simple) => {
        async fn process(ctx: &patternsleuth::resolvers::AsyncContext<'_> , patterns: Vec<&str>) -> patternsleuth::resolvers::Result<$name>{
            let futures = ::patternsleuth::resolvers::futures::future::join_all(
                patterns.iter()
                    .map(|p| ctx.scan(::patternsleuth::scanner::Pattern::new(p).unwrap()))
            ).await;

            Ok($name(
                ::patternsleuth::resolvers::ensure_one(futures.into_iter().flatten())?
            ))
        }
    };

    (@emit_body $name:ident, Call) => {

        async fn process(ctx: &patternsleuth::resolvers::AsyncContext<'_> , patterns: Vec<&str>) -> patternsleuth::resolvers::Result<$name>{
            let res = futures::future::join_all(patterns.iter().map(|p| ctx.scan(patternsleuth::scanner::Pattern::new(p).unwrap()))).await;
            
            Ok($name(try_ensure_one(
                res.iter()
                    .flatten()
                    .map(|a| -> Result<usize> { Ok(ctx.image().memory.rip4(*a)?) }))?))
        }
    };

    (@emit_body $name:ident, XrefLast) => {

        async fn process(ctx: &patternsleuth::resolvers::AsyncContext<'_> , patterns: Vec<patternsleuth::scanner::Pattern>) -> patternsleuth::resolvers::Result<$name>{
            
            // let strings = futures::future::join_all(patterns.iter().map(|p| ctx.scan(p.clone()))).await;
            let strings = ctx.scan(patterns.first().unwrap().clone()).await;
            let refs = util::scan_xrefs(ctx, &strings).await;
            let mut fns = util::root_functions(ctx, &refs)?;
            if fns.len() == 2 {
                fns[0] = fns[1]; // FIXME: deque? last?
                fns.pop();
            }
            Ok($name(ensure_one(fns)?))
        }
    };

    (@emit_body $name:ident, Xref) => {

        async fn process(ctx: &patternsleuth::resolvers::AsyncContext<'_> , patterns: Vec<patternsleuth::scanner::Pattern>) -> patternsleuth::resolvers::Result<$name>{
            
            // let strings = futures::future::join_all(patterns.iter().map(|p| ctx.scan(p.clone()))).await;
            let strings = ctx.scan(patterns.first().unwrap().clone()).await;
            let refs = util::scan_xrefs(ctx, &strings).await;
            let mut fns = util::root_functions(ctx, &refs)?;
            Ok($name(ensure_one(fns.first().unwrap().clone())?))
        }
    };
    
    // Fallback: mode not matched. Call user macro with name $mode
    (@emit_body $name:ident, $mode:ident) => {
        $mode![@emit_body $name];
        // async fn process(ctx: &patternsleuth::resolvers::AsyncContext<'_> , patterns: Vec<patternsleuth::scanner::Pattern>) -> patternsleuth::resolvers::Result<$name>{
            
        // }
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