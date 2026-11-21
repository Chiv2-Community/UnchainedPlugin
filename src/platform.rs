//! Storefront ownership and adapters for the generic resolver toolkit.

use std::sync::OnceLock;

#[unchained_macros::resolver_selection_key(require = [STEAM, EGS])]
#[derive(Debug, Hash, serde::Serialize, serde::Deserialize)]
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

pub fn supports_platform(name: &str, platform: PlatformType) -> bool {
    unchained_macros::supports_selection(name, platform)
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
            "EGS" => Ok(Self::EGS),
            "STEAM" => Ok(Self::STEAM),
            "XBOX" => Ok(Self::XBOX),
            "OTHER" => Ok(Self::OTHER),
            _ => Err(()),
        }
    }
}

// Preserve native declarations' syntax; active + OTHER is plugin policy.
macro_rules! define_pattern_resolver {
    ($name:ident, $mode:ident, { $($key:ident: $entry:tt),+ $(,)? }) => {
        unchained_macros::define_pattern_resolver!($name, $mode,
            select [$crate::current_platform(), $crate::PlatformType::OTHER],
            { $($crate::PlatformType::$key: $entry),+ });
    };
    ($name:ident, { $($key:ident: $entry:tt),+ $(,)? }, |$ctx:ident, $patterns:ident| $body:block) => {
        unchained_macros::define_pattern_resolver!($name,
            select [$crate::current_platform(), $crate::PlatformType::OTHER],
            { $($crate::PlatformType::$key: $entry),+ },
            |$ctx, $patterns| $body);
    };
    ($name:ident, { $($key:ident: $entry:tt),+ $(,)? }) => {
        define_pattern_resolver!($name, Simple, { $($key: $entry),+ });
    };
    ($($resolver:tt)+) => { unchained_macros::define_pattern_resolver!($($resolver)+); };
}

#[macro_export]
macro_rules! CREATE_PATCH_PLATFORM {
    ($platform:ident, $name:ident $(@ $tag:ident)?, $extra:literal, $op:ident, $val:expr, IF $cond:block) => {
        unchained_macros::CREATE_PATCH!($name $(@ $tag)?, $extra, $op, $val, IF {
            $crate::PLATFORM.get().is_some_and(|key| *key == $crate::PlatformType::$platform) && $cond
        });
    };
    ($platform:ident, $name:ident $(@ $tag:ident)?, $extra:literal, $op:ident, $val:expr) => {
        $crate::CREATE_PATCH_PLATFORM!($platform, $name $(@ $tag)?, $extra, $op, $val, IF { true });
    };
    ($platform:ident, $name:ident, $op:ident, $val:expr) => {
        unchained_macros::CREATE_PATCH!($name, $op, $val, IF {
            $crate::PLATFORM.get().is_some_and(|key| *key == $crate::PlatformType::$platform)
        });
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    define_pattern_resolver!(FixtureSteamOnly, {
        STEAM: ["AA BB"], EGS: None, XBOX: None, OTHER: None,
    });
    define_pattern_resolver!(FixtureEpicCall, Call, {
        EGS: ["E8 | ?? ?? ?? ??"], STEAM: None, XBOX: None, OTHER: None,
    });

    #[test_case::test_matrix(
        [("FixtureSteamOnly", Some(PlatformType::STEAM)), ("FixtureEpicCall", Some(PlatformType::EGS)), ("FixtureShared", None)],
        [PlatformType::STEAM, PlatformType::EGS, PlatformType::XBOX, PlatformType::OTHER]
    )]
    fn adapters_preserve_storefront_restrictions(
        (name, restriction): (&str, Option<PlatformType>), platform: PlatformType,
    ) {
        assert_eq!(supports_platform(name, platform),
            restriction.is_none_or(|expected| expected == platform));
    }

    #[test_case::test_case("steam", PlatformType::STEAM; "steam")]
    #[test_case::test_case("EgS", PlatformType::EGS; "egs")]
    #[test_case::test_case("XBOX", PlatformType::XBOX; "xbox")]
    #[test_case::test_case("OTHER", PlatformType::OTHER; "other")]
    fn names_and_cache_serialization_remain_compatible(name: &str, expected: PlatformType) {
        assert_eq!(name.parse::<PlatformType>().unwrap(), expected);
        assert_eq!(serde_json::to_string(&expected).unwrap(), format!("\"{expected}\""));
    }
}
