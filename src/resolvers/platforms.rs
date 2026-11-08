use super::PlatformType;

/// Platforms declared by a pattern resolver; OTHER means a shared signature.
pub struct ResolverPlatforms {
    pub name: &'static str,
    pub platforms: &'static [PlatformType],
}

inventory::collect!(ResolverPlatforms);

/// Select platform-specific patterns first, followed by shared patterns.
pub fn select_patterns<T: Clone>(platform: PlatformType, groups: &[(PlatformType, &[T])]) -> Vec<T> {
    let specific = groups.iter().filter(|(key, _)| *key == platform);
    let shared = groups.iter().filter(|(key, _)| {
        *key == PlatformType::OTHER && platform != PlatformType::OTHER
    });
    specific.chain(shared)
        .flat_map(|(_, patterns)| patterns.iter().cloned())
        .collect()
}

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
    define_pattern_resolver!(@register_platforms TestSteamOnly, [STEAM], [EGS]);
    define_pattern_resolver!(@register_platforms TestEpicOnly, [EGS], [STEAM]);
    define_pattern_resolver!(@register_platforms TestBoth, [STEAM, EGS], []);
    define_pattern_resolver!(@register_platforms TestShared, [OTHER], []);

    #[test]
    fn pattern_selection_orders_specific_before_shared_and_excludes_other_platforms() {
        let groups: &[(PlatformType, &[&str])] = &[
            (PlatformType::OTHER, &["shared"]),
            (PlatformType::EGS, &["epic"]),
            (PlatformType::STEAM, &["steam", "fallback"]),
        ];
        assert_eq!(select_patterns(PlatformType::STEAM, groups), ["steam", "fallback", "shared"]);
        assert_eq!(select_patterns(PlatformType::EGS, groups), ["epic", "shared"]);
        assert_eq!(select_patterns(PlatformType::XBOX, groups), ["shared"]);
        assert_eq!(select_patterns(PlatformType::OTHER, groups), ["shared"]);
        assert!(select_patterns::<&str>(PlatformType::STEAM, &[]).is_empty());
    }

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
