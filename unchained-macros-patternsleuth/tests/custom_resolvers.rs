use binary_bloodhound::BinaryMatcher;
use std::sync::atomic::{AtomicUsize, Ordering};
use unchained_macros_patternsleuth::define_custom_resolver;

mod support;

use support::{image, initialize_selection, Flavor, BASE, TEXT};

static EVALUATIONS: AtomicUsize = AtomicUsize::new(0);

define_custom_resolver!(RootAddress, {

    BinaryMatcher::patterns(["AA BB"])?
        .root_functions()
        .addresses()
        .ensure_one()
});

define_custom_resolver!(CachedAddress, {

    BinaryMatcher::from_sync(|()| {

        EVALUATIONS.fetch_add(1, Ordering::SeqCst);

        Ok(vec![BASE])
    })
    .map(|node| *node.value + 0x1000)
    .ensure_one()
});

define_custom_resolver!(InvalidPattern, BinaryMatcher::patterns(["not hex"])?.ensure_one());

define_custom_resolver!(SelectedAddress, using Flavor, |flavor| {
    let pattern = match flavor {
        Flavor::First => "AA BB",
        Flavor::Second => "11 22",
        Flavor::Shared => "not hex",
    };

    BinaryMatcher::patterns([pattern])?.ensure_one()
});

#[test_case::test_case(&[0x10]; "one interior match")]
#[test_case::test_case(&[0x10, 0x20]; "two matches in the same function")]

fn interior_patterns_resolve_to_one_function_entry(matches: &[usize]) {

    assert_eq!(image(matches).resolve(RootAddress::resolver()).unwrap().0, TEXT);
}

#[test_case::test_case(&[]; "missing match")]
#[test_case::test_case(&[0x10, 0x50]; "ambiguous function entries")]

fn missing_or_ambiguous_addresses_fail_with_resolver_context(matches: &[usize]) {

    let error = image(matches).resolve(RootAddress::resolver()).unwrap_err().to_string();

    assert!(error.contains("RootAddress"), "{error}");
}

#[test]

fn malformed_patterns_propagate_errors() {

    let error = image(&[]).resolve(InvalidPattern::resolver()).unwrap_err().to_string();

    assert!(error.contains("InvalidPattern"), "{error}");

    assert!(error.contains("invalid pattern"), "{error}");
}

#[test]

fn arbitrary_matchers_share_the_resolver_cache_and_preserve_u64_addresses() {

    let image = image(&[]);

    patternsleuth::resolvers::eval(&image, |ctx| {

        Box::pin(async {

            let first = ctx.resolve(CachedAddress::resolver()).await.unwrap();

            let second = ctx.resolve(CachedAddress::resolver()).await.unwrap();

            assert!(std::sync::Arc::ptr_eq(&first, &second));

            assert_eq!(first.0, TEXT);
        })
    });

    assert_eq!(EVALUATIONS.load(Ordering::SeqCst), 1);
}

#[test_case::test_case(Flavor::First, TEXT + 0x10; "first")]
#[test_case::test_case(Flavor::Second, TEXT; "second")]

fn selection_factory_selects_the_matching_signature(flavor: Flavor, expected: u64) {

    if !initialize_selection(flavor) {

        return;
    }

    assert_eq!(image(&[0x10]).resolve(SelectedAddress::resolver()).unwrap().0, expected);
}

#[test]

fn selection_factory_construction_errors_keep_resolver_context() {

    if !initialize_selection(Flavor::Shared) {

        return;
    }

    let error = image(&[]).resolve(SelectedAddress::resolver()).unwrap_err().to_string();

    assert!(error.contains("SelectedAddress"), "{error}");

    assert!(error.contains("invalid pattern"), "{error}");
}
