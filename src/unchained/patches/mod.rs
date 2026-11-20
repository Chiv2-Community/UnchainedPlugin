//! Detours and instruction patches grouped by plugin purpose.

pub mod auth_token_cleanup;
pub mod backend_hooks;
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
pub mod server_browser;
pub mod unchained_integration;
