use serde::Serialize;

pub mod rcon;
#[cfg(feature="server_registration")]
pub mod server_registration;
#[cfg(feature="mod_management")]
#[macro_use]
pub mod mod_management;
pub mod events;
pub mod tokio_runtime;
#[cfg(feature="discord_integration")]
pub mod discord;
pub use discord::DiscordConfig;
pub use discord::initialize_discord_system;

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct Mod {
    pub name: String,
    pub organization: String,
    pub version: String,
    pub object_path: String,
}
unsafe impl Send for Mod {}
unsafe impl Sync for Mod {}
