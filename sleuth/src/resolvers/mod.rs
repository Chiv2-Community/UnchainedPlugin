// #[macro_use]
// pub mod macros;

use once_cell::sync::{Lazy, OnceCell};


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "UPPERCASE")]
#[allow(clippy::upper_case_acronyms)]
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

pub static BASE_ADDR: Lazy<usize> = Lazy::new(|| unsafe {
    windows::Win32::System::LibraryLoader::GetModuleHandleW(None)
        .expect("Failed to get module handle")
        .0 as usize
});


pub type CreateHookFn = unsafe fn(std::collections::HashMap<String, u64>, bool) -> Result<Option<usize>, Box<dyn std::error::Error>>;

pub struct HookRegistration {
    pub name: &'static str,
    pub hook_fn: CreateHookFn,
    pub condition: fn() -> bool,
    // pub auto_activate: bool,
}

pub type RegistrationPredicate = fn() -> bool;
pub type CreatePatchFn = unsafe fn(std::collections::HashMap<String, u64>) -> Result<(), Box<dyn std::error::Error>>;

pub struct PatchRegistration {
    pub name: &'static str,
    pub tag: &'static str,
    pub patch_fn: CreatePatchFn,
    pub enabled_fn: RegistrationPredicate, // The runtime check
}

/// A named group of offsets to help categorize a set of named offsets.
pub struct OffsetRegisty {
    pub name: &'static str,
    pub map: fn() -> std::collections::HashMap<String, u64>,
}

pub mod asset_registry;
pub mod admin_control;
pub mod asset_loading;
pub mod backend_hooks;
pub mod etc_hooks;
pub mod ownership_overrides;
pub mod unchained_integration;
pub mod messages;
pub mod desync;
#[cfg(feature="move-autonomous-desync")]
pub mod desync_tests;

#[macro_use]
pub mod chiv2_macros;
#[macro_use]
pub mod getpost_requests;

inventory::collect!(HookRegistration);
inventory::collect!(PatchRegistration);
inventory::collect!(OffsetRegisty);