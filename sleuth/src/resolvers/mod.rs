#[macro_use]
pub mod macros;

use once_cell::sync::OnceCell;


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PlatformType {
    EGS,
    STEAM,
    XBOX,
    OTHER,
}

pub static PLATFORM: OnceCell<PlatformType> = OnceCell::new();
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

pub static BASE_ADDR: OnceCell<usize> = OnceCell::new();

pub mod asset_registry;
pub mod admin_control;
pub mod asset_loading;
pub mod backend_hooks;
pub mod etc_hooks;
pub mod ownership_overrides;
pub mod unchained_integration;