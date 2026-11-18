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

mod platforms;
pub use platforms::{supports_platform, ResolverPlatform};

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

/// Attach a native-registry hook. Takes no offsets: it resolves its target from a class's
/// RegisterNatives table at call time.
pub type LateHookAttachFn = unsafe fn() -> Result<(), Box<dyn std::error::Error>>;

/// A UFunction native-thunk hook that resolves its target from a reflected class's RegisterNatives
/// table, so it must attach LATE (postinit, after `init_globals`) rather than through the preinit
/// `HookRegistration` inventory. Submitted by `kismet_exec_observe!` / `kismet_exec_forward!` and
/// drained once in `postinit_rustlib`.
pub struct LateHookRegistration {
    pub class: &'static str,
    pub native: &'static str,
    pub attach: LateHookAttachFn,
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
pub mod loading_screens;
pub mod backend_hooks;
pub mod config_overrides;
pub mod etc_hooks;
pub mod ownership_overrides;
pub mod unchained_integration;
pub mod messages;
#[cfg(any(feature = "blueprint_printstring_log", feature = "kismet_server_identity_hooks"))]
pub mod kismet_natives;
pub mod desync;
pub(crate) mod steam_auth;
mod playfab_auth;
#[cfg(feature="move-autonomous-desync")]
pub mod desync_tests;

#[macro_use]
pub mod chiv2_macros;
#[macro_use]
pub mod getpost_requests;
pub mod gameplay;
pub mod pak_mount;

inventory::collect!(HookRegistration);
inventory::collect!(LateHookRegistration);
inventory::collect!(PatchRegistration);
inventory::collect!(OffsetRegisty);

/// Attach every `LateHookRegistration` (reflection-resolved hooks that need the live UObject
/// system). **Idempotent** — the first call does the work, later calls are no-ops — so it can be
/// driven from the earliest available trigger (the `FEngineLoop::Init` entry hook) with the
/// `ENGINE_READY` wait as a fallback, without double-initializing a detour.
pub fn attach_late_hooks() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        for hook in inventory::iter::<LateHookRegistration> {
            match unsafe { (hook.attach)() } {
                Ok(()) => crate::sinfo!("☑ {}::{} (Kismet native)", hook.class, hook.native),
                Err(e) => crate::serror!("☐ {}::{}: {}", hook.class, hook.native, e),
            }
        }
    });
}
