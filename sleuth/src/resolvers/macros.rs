
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
    // --- Per-Platform Map ---
    ($name:ident, { $( $platform:ident : [ $( $pattern:expr ),+ $(,)? ] ),+ $(,)? }) => {
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
            // println!("combined {:?}", patterns);

            let futures = ::patternsleuth::resolvers::futures::future::join_all(
                patterns.iter()
                    .map(|p| ctx.scan(::patternsleuth::scanner::Pattern::new(p).unwrap()))
            ).await;

            Ok($name(
                ::patternsleuth::resolvers::ensure_one(futures.into_iter().flatten())?
            ))
        });
    };

    // --- Flat Pattern List ---
    ($name:ident, [ $( $pattern:expr ),+ $(,)? ]) => {
        #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
        #[allow(non_camel_case_types)]
        pub struct $name(pub usize);

        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |ctx| async {
            let patterns = [ $( $pattern ),+ ];
            let futures = ::patternsleuth::resolvers::futures::future::join_all(
                patterns.iter()
                    .map(|p| ctx.scan(::patternsleuth::scanner::Pattern::new(p).unwrap()))
            ).await;

            Ok($name(::patternsleuth::resolvers::ensure_one(futures.into_iter().flatten())?))
        });
    };
}


// #[macro_export]
// macro_rules! define_pattern_resolver {
//     ($name:ident, [ $( $pattern:expr ),+ $(,)? ]) => {
//         #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
//         #[allow(non_camel_case_types)]
//         pub struct $name(pub usize);

//         ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |ctx| async {
//             let patterns = [ $( $pattern ),+ ];
//             let res = ::patternsleuth::resolvers::futures::future::join_all(
//                 patterns.iter().map(|p| ctx.scan(::patternsleuth::scanner::Pattern::new(p).unwrap()))
//             ).await;
//             Ok($name(::patternsleuth::resolvers::ensure_one(res.into_iter().flatten())?))
//         });
//     };
// }