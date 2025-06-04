

#[macro_export]
macro_rules! define_pattern_resolver {
    ($name:ident, [ $( $pattern:expr ),+ $(,)? ]) => {
        #[derive(Debug, PartialEq)]
        #[cfg_attr(
            feature = "serde-resolvers",
            derive(serde::Serialize, serde::Deserialize)
        )]
        #[allow(non_camel_case_types)]
        pub struct $name(pub usize);

        patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |ctx| async {
            let patterns = [ $( $pattern ),+ ];
            let res = ::patternsleuth::resolvers::futures::future::join_all(
                patterns.iter().map(|p| ctx.scan(::patternsleuth::scanner::Pattern::new(p).unwrap()))
            ).await;
            Ok($name(::patternsleuth::resolvers::ensure_one(res.into_iter().flatten())?))
        });
    };
}