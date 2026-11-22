mod support;

use object::SectionKind;
use patternsleuth::{NamedMemorySection, image::Image, scanner::Pattern};
use support::{BASE, TEXT, image, initialize_selection, Flavor};
use unchained_macros_patternsleuth::define_pattern_resolver;
use std::sync::atomic::{AtomicUsize, Ordering};

static PATTERN_EVALUATIONS: AtomicUsize = AtomicUsize::new(0);
static BODY_EVALUATIONS: AtomicUsize = AtomicUsize::new(0);

fn shared_pattern() -> &'static str {
    PATTERN_EVALUATIONS.fetch_add(1, Ordering::SeqCst);
    "11 22"
}

define_pattern_resolver!(Unique, ["AA BB", "11 22"]);
define_pattern_resolver!(First, First, ["AA BB", "11 22"]);
define_pattern_resolver!(CallTarget, Call, ["E8 | ?? ?? ?? ??"]);
define_pattern_resolver!(FirstReference, XrefFirst, [
    Pattern::from_bytes(b"primary\0".to_vec()).unwrap(),
    Pattern::from_bytes(b"secondary\0".to_vec()).unwrap(),
]);
define_pattern_resolver!(LastReference, XrefLast, [
    Pattern::from_bytes(b"primary\0".to_vec()).unwrap(),
    Pattern::from_bytes(b"secondary\0".to_vec()).unwrap(),
]);

mod nested {
    use unchained_macros_patternsleuth::define_pattern_resolver;
    use super::support::Flavor;

    #[unchained_macros_core::resolver_selection_key(fallback = Declared)]
    enum Fallback { Missing, Declared }
    impl unchained_macros_core::ResolverSelector for Fallback {
        fn current() -> Self { Self::Missing }
    }

    define_pattern_resolver!(Dictionary, {
        Flavor::First: ["AA BB"],
        Flavor::Second: ["CC DD"],
        Flavor::Shared: ["11 22"],
    });
    define_pattern_resolver!(InlineDictionary, {
        Flavor::First: ["AA BB"],
        Flavor::Second: ["CC DD"],
        Flavor::Shared: ["11 22"],
    }, |_ctx, patterns| { Ok(patterns.len() as u64) });
    define_pattern_resolver!(Ordered, First, {
        Fallback::Declared: ["11 22"],
    });
    define_pattern_resolver!(Disabled, {
        Flavor::First: None,
        Flavor::Second: [super::shared_pattern()],
        Flavor::Shared: [super::shared_pattern()],
    });
    define_pattern_resolver!(DisabledInline, {
        Flavor::First: None,
        Flavor::Second: [super::shared_pattern()],
        Flavor::Shared: [super::shared_pattern()],
    }, |_ctx, patterns| {
        super::BODY_EVALUATIONS.fetch_add(1, super::Ordering::SeqCst);
        Ok(patterns.len() as u64)
    });
    define_pattern_resolver!(AllDisabled, {
        Flavor::First: None, Flavor::Second: None,
    });
    define_pattern_resolver!(AllDisabledXref, XrefFirst, {
        Flavor::First: None, Flavor::Second: None,
    });
    define_pattern_resolver!(AllDisabledInline, {
        Flavor::First: None, Flavor::Second: None,
    }, |_ctx, patterns| { Ok(patterns.len() as u64) });
}

#[test]
fn all_none_dictionaries_compile_and_cannot_resolve() {
    if !initialize_selection(Flavor::First) { return; }
    let image = image(&[]);
    assert!(image.resolve(nested::AllDisabled::resolver()).unwrap_err().to_string().contains("disabled"));
    assert!(image.resolve(nested::AllDisabledXref::resolver()).unwrap_err().to_string().contains("disabled"));
    assert!(image.resolve(nested::AllDisabledInline::resolver()).unwrap_err().to_string().contains("disabled"));
}

#[test_case::test_case(Flavor::First, false; "none overrides fallback patterns")]
#[test_case::test_case(Flavor::Second, true; "explicit list stays enabled")]
fn disabled_keys_skip_pattern_construction_and_inline_bodies(flavor: Flavor, enabled: bool) {
    if !initialize_selection(flavor) { return; }
    let snapshot = unchained_macros_core::SelectorSnapshot::from_selection(flavor).unwrap();
    assert_eq!(snapshot.supports_resolver_for::<unchained_macros_patternsleuth::PatternsleuthResolver>("Disabled"), enabled);
    assert_eq!(snapshot.supports_resolver_for::<unchained_macros_patternsleuth::PatternsleuthResolver>("DisabledInline"), enabled);
    let image = image(&[]);
    let standard = image.resolve(nested::Disabled::resolver());
    let inline = image.resolve(nested::DisabledInline::resolver());
    if enabled {
        assert_eq!(standard.unwrap().0, TEXT);
        assert_eq!(inline.unwrap().0, 1);
    } else {
        assert!(standard.unwrap_err().to_string().contains("disabled"));
        assert!(inline.unwrap_err().to_string().contains("disabled"));
    }
    assert_eq!(PATTERN_EVALUATIONS.load(Ordering::SeqCst), if enabled { 2 } else { 0 });
    assert_eq!(BODY_EVALUATIONS.load(Ordering::SeqCst), usize::from(enabled));
}

#[test]
fn simple_requires_uniqueness_while_first_preserves_pattern_order() {
    let image = image(&[0x10]);
    assert!(image.resolve(Unique::resolver()).is_err());
    assert_eq!(image.resolve(First::resolver()).unwrap().0, TEXT + 0x10);
}

#[test]
fn call_follows_the_marked_displacement() {
    let mut text = vec![0x90; 0x80];
    text[8] = 0xE8;
    text[9..13].copy_from_slice(&((0x40i32) - 13).to_le_bytes());
    let image = image(&[]).with_text(text).unwrap();
    assert_eq!(image.resolve(CallTarget::resolver()).unwrap().0, TEXT + 0x40);
}

fn references_image(references: bool) -> Image<'static> {
    let mut text = vec![0x90; 0x80];
    if references {
        for (offset, target) in [(0x10, BASE + 0x4000), (0x50, BASE + 0x4000), (0x30, BASE + 0x4010)] {
            text[offset..offset + 3].copy_from_slice(&[0x48, 0x8D, 0x0D]);
            let displacement = (target as i64 - (TEXT + offset as u64 + 7) as i64) as i32;
            text[offset + 3..offset + 7].copy_from_slice(&displacement.to_le_bytes());
        }
    }
    let mut data = b"primary\0".to_vec();
    data.resize(16, 0);
    data.extend_from_slice(b"secondary\0");
    image(&[]).with_text(text).unwrap().with_section(NamedMemorySection::new(
        ".rdata".into(), BASE + 0x4000, SectionKind::ReadOnlyData, data,
    ))
}

#[test]
fn xref_modes_select_root_functions_from_only_the_first_pattern() {
    let image = references_image(true);
    assert_eq!(image.resolve(FirstReference::resolver()).unwrap().0, TEXT);
    assert_eq!(image.resolve(LastReference::resolver()).unwrap().0, TEXT + 0x40);
    let missing = references_image(false);
    assert!(missing.resolve(FirstReference::resolver()).is_err());
    assert!(missing.resolve(LastReference::resolver()).is_err());
}

#[test_case::test_case(Flavor::First, Some(TEXT + 0x10); "first only")]
#[test_case::test_case(Flavor::Second, None; "failed scan does not fall back")]
#[test_case::test_case(Flavor::Shared, Some(TEXT); "explicit fallback key")]
fn selection_dictionary_uses_first_declared_key(flavor: Flavor, expected: Option<u64>) {
    if !initialize_selection(flavor) { return; }
    let image = image(&[0x10]);
    assert_eq!(image.resolve(nested::InlineDictionary::resolver()).unwrap().0, 1);
    let result = image.resolve(nested::Dictionary::resolver());
    match expected {
        Some(address) => assert_eq!(result.unwrap().0, address),
        None => assert!(result.is_err()),
    }
    assert_eq!(image.resolve(nested::Ordered::resolver()).unwrap().0, TEXT);
}
