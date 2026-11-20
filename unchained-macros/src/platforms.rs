//! Storefront selection and explicit resolver restrictions.

use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "UPPERCASE")]
#[allow(clippy::upper_case_acronyms)]
pub enum PlatformType {
    EGS,
    STEAM,
    XBOX,
    OTHER,
}

pub static PLATFORM: OnceLock<PlatformType> = OnceLock::new();

pub fn current_platform() -> PlatformType {
    *PLATFORM.get().expect("Platform not initialized")
}

impl std::fmt::Display for PlatformType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::str::FromStr for PlatformType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "EGS" => Ok(PlatformType::EGS),
            "STEAM" => Ok(PlatformType::STEAM),
            "XBOX" => Ok(PlatformType::XBOX),
            "OTHER" => Ok(PlatformType::OTHER),
            _ => Err(()),
        }
    }
}

/// Explicit platform restriction shared by a resolver and its hooks and patches.
pub struct ResolverPlatform {
    pub name: &'static str,
    pub platform: PlatformType,
}

inventory::collect!(ResolverPlatform);

/// Resolvers without an explicit restriction keep their existing applicability.
pub fn supports_platform(name: &str, platform: PlatformType) -> bool {
    inventory::iter::<ResolverPlatform>
        .into_iter()
        .filter(|resolver| resolver.name == name)
        .all(|resolver| resolver.platform == platform)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{define_pattern_resolver, define_single_platform_pattern_resolver};
    use crate::signature::Call_signature;
    use patternsleuth::resolvers::ensure_one;
    define_single_platform_pattern_resolver!(STEAM, TestSteamOnly, ["AA BB"]);

    define_single_platform_pattern_resolver!(EGS, TestEpicOnly, Simple, ["CC DD"]);

    define_single_platform_pattern_resolver!(STEAM, TestSteamCall, [Call_signature("E8 | ?? ?? ?? ??")]);

    define_single_platform_pattern_resolver!(STEAM, TestInlineSteamOnly, ["EE FF"], |ctx, patterns| {
        let matches = ctx.scan(patternsleuth::scanner::Pattern::new(patterns[0]).unwrap()).await;
        ensure_one(matches)
    });

    define_pattern_resolver!(TestShared, ["11 22"]);

    mod nested {
        use crate::define_pattern_resolver;
        define_pattern_resolver!(TestNestedPlatform, {
            STEAM: ["AA BB"],
            OTHER: ["CC DD"]
        });

        define_pattern_resolver!(TestNestedSimplePlatform, Simple, {
            EGS: ["11 22"],
            OTHER: ["33 44"]
        });

    }

    #[test]
    fn platform_resolvers_register_from_nested_modules() {
        for name in ["TestNestedPlatform", "TestNestedSimplePlatform"] {
            assert!(patternsleuth::resolvers::resolvers().any(|resolver| resolver.name == name));
        }
    }

    #[test]
    fn single_platform_resolver_is_skipped_on_other_platforms() {
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
    fn inline_resolver_keeps_single_platform_applicability() {
        assert!(supports_platform("TestInlineSteamOnly", PlatformType::STEAM));
        assert!(!supports_platform("TestInlineSteamOnly", PlatformType::EGS));
    }

    #[test]
    fn typed_signature_keeps_single_platform_applicability() {
        assert!(supports_platform("TestSteamCall", PlatformType::STEAM));
        assert!(!supports_platform("TestSteamCall", PlatformType::EGS));
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
