use resolvers::PlatformType;
use sleuth_macros::resolver_platforms;

#[path = "../../src/resolvers/platforms.rs"]
mod platform_registry;

mod resolvers {
    pub use super::platform_registry::ResolverPlatforms;

    #[derive(Clone, Copy, PartialEq)]
    pub enum PlatformType {
        STEAM,
        EGS,
        XBOX,
        OTHER,
    }
}

resolver_platforms!(SteamOnly, [STEAM], [EGS]);
resolver_platforms!(EpicOnly, [EGS], [STEAM]);
resolver_platforms!(Both, [STEAM, EGS], []);
resolver_platforms!(Shared, [OTHER], []);

#[test]
fn steam_only_is_skipped_on_other_platforms_even_when_warning_is_suppressed() {
    assert!(platform_registry::supports_platform(
        "SteamOnly",
        PlatformType::STEAM
    ));
    assert!(!platform_registry::supports_platform(
        "SteamOnly",
        PlatformType::EGS
    ));
    assert!(!platform_registry::supports_platform(
        "SteamOnly",
        PlatformType::XBOX
    ));
    assert!(!platform_registry::supports_platform(
        "SteamOnly",
        PlatformType::OTHER
    ));
}

#[test]
fn epic_only_is_skipped_on_steam() {
    assert!(platform_registry::supports_platform(
        "EpicOnly",
        PlatformType::EGS
    ));
    assert!(!platform_registry::supports_platform(
        "EpicOnly",
        PlatformType::STEAM
    ));
}

#[test]
fn explicit_coverage_does_not_enable_unlisted_platforms() {
    assert!(platform_registry::supports_platform(
        "Both",
        PlatformType::STEAM
    ));
    assert!(platform_registry::supports_platform(
        "Both",
        PlatformType::EGS
    ));
    assert!(!platform_registry::supports_platform(
        "Both",
        PlatformType::XBOX
    ));
}

#[test]
fn shared_and_unregistered_resolvers_keep_existing_applicability() {
    for platform in [
        PlatformType::STEAM,
        PlatformType::EGS,
        PlatformType::XBOX,
        PlatformType::OTHER,
    ] {
        assert!(platform_registry::supports_platform("Shared", platform));
        assert!(platform_registry::supports_platform(
            "Unregistered",
            platform
        ));
    }
}
