use serde::Serialize;

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct Mod {
    pub name: String,
    pub organization: String,
    pub version: String,
    pub object_path: String,
}

#[derive(Clone, Debug)]
pub struct ServerStatus {
    pub name: String,
    pub description: String,
    pub password_protected: bool,
    pub current_map: String,
    pub player_count: i32,
    pub max_players: i32,
    pub mods: Vec<Mod>,
    pub active_mods: Vec<Mod>,
}
