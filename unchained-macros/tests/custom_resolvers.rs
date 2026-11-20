use binary_bloodhound::BinaryMatcher;
use object::SectionKind;
use patternsleuth::{
    Memory, NamedMemorySection,
    image::{Image, ImageType, pe::PEImage},
};
use std::sync::atomic::{AtomicUsize, Ordering};
use unchained_macros::define_custom_resolver;

const BASE: u64 = 0x1_4000_0000;
const TEXT: u64 = BASE + 0x1000;
static EVALUATIONS: AtomicUsize = AtomicUsize::new(0);

use unchained_macros::{PlatformType, PLATFORM};

// Storefront selection is process-wide and immutable. Isolate each selection.
fn initialize_platform(platform: PlatformType) -> bool {
    if std::env::var_os("UNCHAINED_PLATFORM_FIXTURE").is_some() {
        PLATFORM.set(platform).unwrap();
        return true;
    }
    let thread = std::thread::current();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", thread.name().unwrap(), "--nocapture"])
        .env("UNCHAINED_PLATFORM_FIXTURE", "1")
        .status().unwrap();
    assert!(status.success());
    false
}

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

define_custom_resolver!(PlatformAddress, |platform| {
    let pattern = match platform {
        PlatformType::STEAM => "AA BB",
        PlatformType::EGS => "11 22",
        PlatformType::XBOX | PlatformType::OTHER => "not hex",
    };
    BinaryMatcher::patterns([pattern])?.ensure_one()
});

fn image(matches: &[usize]) -> Image<'static> {
    let mut text = vec![0x90; 0x80];
    text[..2].copy_from_slice(&[0x11, 0x22]);
    for &offset in matches {
        text[offset..offset + 2].copy_from_slice(&[0xAA, 0xBB]);
    }
    let pdata: Vec<u8> = [0x1000u32, 0x1040, 0x3000, 0x1040, 0x1080, 0x3004]
        .into_iter().flat_map(u32::to_le_bytes).collect();
    Image::from_memory(
        BASE,
        Memory::from_sections(vec![
            NamedMemorySection::new(".text".into(), TEXT, SectionKind::Text, text),
            NamedMemorySection::new(".pdata".into(), BASE + 0x2000, SectionKind::ReadOnlyData, pdata),
            NamedMemorySection::new(".xdata".into(), BASE + 0x3000, SectionKind::ReadOnlyData, vec![1, 0, 0, 0, 1, 0, 0, 0]),
        ]),
        ImageType::PEImage(PEImage {
            exception_directory_range: BASE + 0x2000..BASE + 0x2018,
            exception_children_cache: Default::default(),
        }),
    )
}

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
    patternsleuth::resolvers::eval(&image, |ctx| Box::pin(async {
        let first = ctx.resolve(CachedAddress::resolver()).await.unwrap();
        let second = ctx.resolve(CachedAddress::resolver()).await.unwrap();
        assert!(std::sync::Arc::ptr_eq(&first, &second));
        assert_eq!(first.0, TEXT);
    }));
    assert_eq!(EVALUATIONS.load(Ordering::SeqCst), 1);
}

#[test_case::test_case(PlatformType::STEAM, TEXT + 0x10; "steam")]
#[test_case::test_case(PlatformType::EGS, TEXT; "egs")]
fn platform_factory_selects_the_matching_signature(platform: PlatformType, expected: u64) {
    if !initialize_platform(platform) { return; }
    assert_eq!(image(&[0x10]).resolve(PlatformAddress::resolver()).unwrap().0, expected);
}

#[test]
fn platform_factory_construction_errors_keep_resolver_context() {
    if !initialize_platform(PlatformType::OTHER) { return; }
    let error = image(&[]).resolve(PlatformAddress::resolver()).unwrap_err().to_string();
    assert!(error.contains("PlatformAddress"), "{error}");
    assert!(error.contains("invalid pattern"), "{error}");
}

#[test]
fn custom_resolvers_register_with_the_standard_inventory() {
    for name in ["RootAddress", "CachedAddress", "InvalidPattern", "PlatformAddress"] {
        assert!(patternsleuth::resolvers::resolvers().any(|resolver| resolver.name == name));
    }
}
