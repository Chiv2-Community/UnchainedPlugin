/// Define a cached, registered resolver from a single-address Bloodhound matcher.
///
/// The matcher must take no input and resolve to `ExactlyOne` `u64` address.
/// Use `.ensure_one()` to reject missing or ambiguous candidates.
///
/// `define_custom_resolver!(Name, with(expression), |selection| { ... })`
/// evaluates consumer-supplied context when the resolver runs.
/// The body may use `?`, just like the matcher-expression form.
///
/// A single-address matcher is accepted:
/// ```
/// # fn main() {}
/// unchained_macros_patternsleuth::define_custom_resolver!(One,
///     binary_bloodhound::BinaryMatcher::address(1).ensure_one());
/// ```
///
/// Variable-cardinality matchers are rejected:
/// ```compile_fail,E0308
/// # fn main() {}
/// unchained_macros_patternsleuth::define_custom_resolver!(Many,
///     binary_bloodhound::BinaryMatcher::candidates([1, 2]));
/// ```
///
/// The final value must be an address:
/// ```compile_fail,E0308
/// # fn main() {}
/// unchained_macros_patternsleuth::define_custom_resolver!(NotAddress,
///     binary_bloodhound::BinaryMatcher::address(1).map(|_| "text").ensure_one());
/// ```
#[macro_export]

macro_rules! define_custom_resolver {
    // Adapt a dependency's address resolver without string-based inventory lookup.
    ($name:ident, delegate $resolver:path $(,)?) => {
        $crate::__pattern_resolver!(@emit_header $name);
        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |ctx| async {
            Ok(Self(ctx.resolve(<$resolver>::resolver()).await?.0))
        });
    };

    ($name:ident, using $selector:ty, |$key:ident| $matcher:expr $(,)?) => {
        $crate::__private::inventory::submit! { $crate::__private::SelectorProvider::of::<$selector>() }
        $crate::__pattern_resolver!(@emit_header $name);
        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |ctx| async {
            let snapshot = $crate::__private::context_snapshot(ctx).await?;
            let $key = snapshot.get::<$selector>().map_err(::patternsleuth::resolvers::ResolveError::new_msg)?;
            Ok(Self($crate::__private::single_address(($matcher).resolve(ctx).await?)))
        });
    };

    ($name:ident, with($selection:expr), |$key:ident| $matcher:expr $(,)?) => {
        $crate::define_custom_resolver!($name, {
            let $key = $selection;
            $matcher
        });
    };

    ($name:ident, $matcher:expr $(,)?) => {
        $crate::__pattern_resolver!(@emit_header $name);
        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |ctx| async {
            Ok(Self($crate::__private::single_address(($matcher).resolve(ctx).await?)))
        });
    };
}

#[doc(hidden)]

pub fn single_address<P>(node: binary_bloodhound::Trace<u64, P>) -> u64 {

    *node.value
}
