/// Define a cached, registered resolver from a single-address Bloodhound matcher.
///
/// The matcher must take no input and resolve to `ExactlyOne` `u64` address.
/// Use `.ensure_one()` to reject missing or ambiguous candidates.
///
/// `define_custom_resolver!(Name, |platform| { ... })` supplies the consuming
/// plugin's `crate::resolvers::current_platform()` when the resolver runs.
/// The body may use `?`, just like the matcher-expression form.
///
/// Variable-cardinality matchers are rejected:
/// ```compile_fail
/// unchained_macros::define_custom_resolver!(Many,
///     binary_bloodhound::BinaryMatcher::candidates([1, 2]));
/// ```
///
/// The final value must be an address:
/// ```compile_fail
/// unchained_macros::define_custom_resolver!(NotAddress,
///     binary_bloodhound::BinaryMatcher::address(1).map(|_| "text").ensure_one());
/// ```
#[macro_export]
macro_rules! define_custom_resolver {
    ($name:ident, |$platform:ident| $matcher:expr $(,)?) => {
        $crate::define_custom_resolver!($name, {
            let $platform = crate::resolvers::current_platform();
            $matcher
        });
    };
    ($name:ident, $matcher:expr $(,)?) => {
        $crate::define_pattern_resolver!(@emit_header $name);
        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |ctx| async {
            Ok(Self($crate::__private::single_address(($matcher).resolve(ctx).await?)))
        });
    };
}

#[doc(hidden)]
pub fn single_address<P>(node: binary_bloodhound::Trace<u64, P>) -> u64 {
    *node.value
}
