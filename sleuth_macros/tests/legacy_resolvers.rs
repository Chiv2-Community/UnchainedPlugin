use object::SectionKind;
use patternsleuth::{
    image::{pe::PEImage, Image, ImageType},
    resolvers::Singleton,
    Memory, NamedMemorySection,
};
use std::sync::Mutex;

static LOGS: Mutex<Vec<(&str, String)>> = Mutex::new(Vec::new());

// Legacy macros consume the host's storefront API.
#[derive(Clone, Copy)]
enum PlatformType { STEAM, EGS, OTHER }
static PLATFORM: std::sync::OnceLock<PlatformType> = std::sync::OnceLock::new();
fn current_platform() -> PlatformType { *PLATFORM.get().unwrap() }

#[macro_export]
macro_rules! serror {
    ($($arg:tt)*) => { $crate::LOGS.lock().unwrap().push(("error", format!($($arg)*))) };
}

#[macro_export]
macro_rules! sdebug {
    ($($arg:tt)*) => { $crate::LOGS.lock().unwrap().push(("debug", format!($($arg)*))) };
}

#[sleuth_macros::resolver(Simple)]
#[pattern("AA BB")]
#[offset(1)]
#[validate(0xBB)]
fn LegacySimple() {}

#[sleuth_macros::resolver(Call)]
#[pattern("08 00 00 00")]
fn LegacyCall() {}

#[sleuth_macros::resolver(Simple)]
#[pattern("E8 08 00 00 00")]
#[offset(1)]
#[read_rip4]
fn LegacyRip() {}

#[sleuth_macros::resolver(Simple)]
#[pattern("AA BB")]
#[validate(0xCC)]
fn LegacyInvalid() {}

#[sleuth_macros::resolver(Custom)]
fn LegacyBody() {
    let hits = ctx.scan(patternsleuth::scanner::Pattern::new("AA BB").unwrap()).await;
    Ok(hits[0] as usize + 0x1000)
}

#[sleuth_macros::resolver(Custom)]
#[optional]
fn LegacyOptional() {
    Err(patternsleuth::resolvers::ResolveError::new_msg("optional failure"))
}

#[sleuth_macros::custom_resolver]
#[pattern("AA BB")]
#[pattern(STEAM = "CC DD", EGS = "EE FF")]
#[offset(2)]
#[read_rip4]
#[validate(0xAA)]
fn LegacyCustom() -> anyhow::Result<usize> {
    assert_eq!(offset, 2usize);
    assert!(should_read_rip4);
    assert_eq!(validate_expected, Some(0xAA));
    let expected = match current_platform() {
        PlatformType::STEAM => vec!["AA BB", "CC DD"],
        PlatformType::EGS => vec!["AA BB", "EE FF"],
        _ => vec!["AA BB"],
    };
    assert_eq!(active_sigs, expected);
    let hits = ctx.scan(patternsleuth::scanner::Pattern::new(active_sigs[0])?).await;
    return Ok::<usize, anyhow::Error>(hits[0] as usize + offset);
}

#[sleuth_macros::custom_resolver]
fn LegacyFallible() -> anyhow::Result<usize> {
    let value = "not an address".parse::<usize>()
        .map_err(|_| anyhow::anyhow!("custom body failed"))?;
    Ok::<usize, anyhow::Error>(value)
}

#[sleuth_macros::custom_resolver]
fn LegacyEarlyError() -> anyhow::Result<usize> {
    return Err::<usize, anyhow::Error>(anyhow::anyhow!("early failure"));
}

fn image() -> Image<'static> {
    PLATFORM.get_or_init(|| {
        match std::env::var("TEST_LEGACY_PLATFORM").as_deref().unwrap_or("STEAM") {
            "STEAM" => PlatformType::STEAM,
            "EGS" => PlatformType::EGS,
            _ => PlatformType::OTHER,
        }
    });
    Image::from_memory(
        0x1000,
        Memory::from_sections(vec![NamedMemorySection::new(
            ".text".into(),
            0x2000,
            SectionKind::Text,
            vec![0xAA, 0xBB, 0x90, 0xE8, 0x08, 0, 0, 0],
        )]),
        ImageType::PEImage(PEImage {
            exception_directory_range: 0..0,
            exception_children_cache: Default::default(),
        }),
    )
}

#[test]
fn standard_resolvers_use_dependency_addresses_and_modifiers() {
    let image = image();
    assert_eq!(image.resolve(LegacySimple::resolver()).unwrap().get(), Some(0x2001));
    assert_eq!(image.resolve(LegacyCall::resolver()).unwrap().get(), Some(0x2010));
    assert_eq!(image.resolve(LegacyRip::resolver()).unwrap().get(), Some(0x2010));
    assert_eq!(image.resolve(LegacyBody::resolver()).unwrap().get(), Some(0x3000));
    let error = image.resolve(LegacyInvalid::resolver()).unwrap_err().to_string();
    assert!(error.contains("Validation failed: expected 0xCC, found 0xAA"), "{error}");
}

#[test]
fn custom_body_keeps_injected_values_and_early_success() {
    assert_eq!(image().resolve(LegacyCustom::resolver()).unwrap().get(), Some(0x2002));
}

#[test]
fn fallible_bodies_convert_errors_and_keep_logging() {
    let image = image();
    let error = image.resolve(LegacyFallible::resolver()).unwrap_err().to_string();
    assert!(error.contains("custom body failed"), "{error}");
    let error = image.resolve(LegacyEarlyError::resolver()).unwrap_err().to_string();
    assert!(error.contains("early failure"), "{error}");
    assert!(image.resolve(LegacyOptional::resolver()).is_err());
    let logs = LOGS.lock().unwrap();
    assert!(logs.iter().any(|(level, text)| *level == "error"
        && text.contains("Sleuth: LegacyFallible failed:") && text.contains("custom body failed")));
    assert!(logs.iter().any(|(level, text)| *level == "error"
        && text.contains("Sleuth: LegacyEarlyError failed:") && text.contains("early failure")));
    assert!(logs.iter().any(|(level, text)| *level == "debug"
        && text.contains("Sleuth: LegacyOptional failed to resolve:")));
}

#[test]
fn real_singleton_factories_register_names_and_dynamic_addresses() {
    for (name, factory, address) in [
        ("LegacySimple", LegacySimple::dyn_resolver(), 0x2001),
        ("LegacyCustom", LegacyCustom::dyn_resolver(), 0x2002),
    ] {
        assert_eq!(factory.name, name);
        let registered = patternsleuth::resolvers::resolvers()
            .find(|resolver| resolver.name == name).unwrap();
        assert!(std::ptr::eq((registered.getter)(), factory));
        let resolved = patternsleuth::resolvers::resolve_many(&image(), &[registered.getter]);
        assert_eq!(resolved[0].as_ref().unwrap().get(), Some(address));
    }
    assert_eq!(LegacySimple::resolver().name, "LegacySimple");
    assert_eq!(LegacyCustom::resolver().name, "LegacyCustom");
}
