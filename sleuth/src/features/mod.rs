use serde::Serialize;

pub mod rcon;
pub mod commands;
#[cfg(feature="server_registration")]
pub mod server_registration;
#[cfg(feature="mod_management")]
#[macro_use]
pub mod mod_management;

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct Mod {
    pub name: String,
    pub organization: String,
    pub version: String,
    pub object_path: String,
}
unsafe impl Send for Mod {}
unsafe impl Sync for Mod {}