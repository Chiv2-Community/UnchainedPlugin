use serde::Serialize;

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct Mod {
    pub name: String,
    pub organization: String,
    pub version: String,
    pub object_path: String,
}
