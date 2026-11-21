mod support;

use object::SectionKind;
use patternsleuth::{NamedMemorySection, image::Image, scanner::Pattern};
use support::{BASE, TEXT, image, initialize_platform};
use unchained_macros::{PlatformType, define_pattern_resolver};

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
    use unchained_macros::define_pattern_resolver;

    define_pattern_resolver!(Dictionary, {
        STEAM: ["AA BB"],
        EGS: ["CC DD"],
        OTHER: ["11 22"],
    });
    define_pattern_resolver!(InlineDictionary, {
        STEAM: ["AA BB"],
        EGS: ["CC DD"],
        OTHER: ["11 22"],
    }, |_ctx, patterns| { Ok(patterns.len() as u64) });
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

#[test_case::test_case(PlatformType::STEAM; "active and shared matches are ambiguous")]
#[test_case::test_case(PlatformType::EGS; "shared pattern supplies the only match")]
fn platform_dictionary_combines_active_and_shared_patterns(platform: PlatformType) {
    if !initialize_platform(platform) { return; }
    let image = image(&[0x10]);
    assert_eq!(image.resolve(nested::InlineDictionary::resolver()).unwrap().0, 2);
    let result = image.resolve(nested::Dictionary::resolver());
    if platform == PlatformType::STEAM {
        assert!(result.is_err());
    } else {
        assert_eq!(result.unwrap().0, TEXT);
    }
}
