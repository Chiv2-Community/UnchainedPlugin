use super::PlatformType;

/// Platforms declared by a pattern resolver; OTHER means a shared signature.
pub struct ResolverPlatforms {
    pub name: &'static str,
    pub platforms: &'static [PlatformType],
}

inventory::collect!(ResolverPlatforms);

pub fn supports_platform(name: &str, platform: PlatformType) -> bool {
    inventory::iter::<ResolverPlatforms>
        .into_iter()
        .filter(|resolver| resolver.name == name)
        .all(|resolver| {
            resolver.platforms.contains(&platform)
                || resolver.platforms.contains(&PlatformType::OTHER)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sleuth_macros::resolver_platforms;

    resolver_platforms!(TestSteamOnly, [STEAM], [EGS]);
    resolver_platforms!(TestEpicOnly, [EGS], [STEAM]);
    resolver_platforms!(TestBoth, [STEAM, EGS], []);
    resolver_platforms!(TestShared, [OTHER], []);

    #[test]
    fn steam_only_is_skipped_on_other_platforms_even_when_warning_is_suppressed() {
        assert!(supports_platform("TestSteamOnly", PlatformType::STEAM));
        assert!(!supports_platform("TestSteamOnly", PlatformType::EGS));
        assert!(!supports_platform("TestSteamOnly", PlatformType::XBOX));
        assert!(!supports_platform("TestSteamOnly", PlatformType::OTHER));
    }

    #[test]
    fn epic_only_is_skipped_on_steam() {
        assert!(supports_platform("TestEpicOnly", PlatformType::EGS));
        assert!(!supports_platform("TestEpicOnly", PlatformType::STEAM));
    }

    #[test]
    fn explicit_coverage_does_not_enable_unlisted_platforms() {
        assert!(supports_platform("TestBoth", PlatformType::STEAM));
        assert!(supports_platform("TestBoth", PlatformType::EGS));
        assert!(!supports_platform("TestBoth", PlatformType::XBOX));
    }

    #[test]
    fn shared_and_unregistered_resolvers_keep_existing_applicability() {
        for platform in [
            PlatformType::STEAM,
            PlatformType::EGS,
            PlatformType::XBOX,
            PlatformType::OTHER,
        ] {
            assert!(supports_platform("TestShared", platform));
            assert!(supports_platform("TestUnregistered", platform));
        }
    }
}
