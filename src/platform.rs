//! Storefront ownership and adapters for the generic resolver toolkit.

use std::sync::OnceLock;
use unchained_macros::ResolverSelector;

#[unchained_macros::resolver_selection_key(require = [STEAM, EGS], fallback = OTHER)]
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
    PlatformType::current()
}

impl ResolverSelector for PlatformType {
    fn current() -> Self { *PLATFORM.get().expect("Platform not initialized") }
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
