//! Detours and instruction patches grouped by plugin purpose.

pub mod backend_hooks;
#[cfg(feature = "kismet_server_identity_hooks")]
pub mod blueprint_identity;
#[cfg(feature = "blueprint_printstring_log")]
pub mod blueprint_prints;
pub mod config_overrides;
pub mod custom_loading_screens;
pub mod damage_events;
pub mod diagnostics;
pub mod enable_console_commands;
pub mod frontend_patches;
pub mod hosting;
#[cfg(feature = "loose_assets")]
pub mod loose_assets;
pub mod messages;
pub mod networking;
pub mod ownership_overrides;
pub mod pak_mount;
pub mod unchained_integration;
