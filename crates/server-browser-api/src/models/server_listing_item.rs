// Generated from OpenAPI anyOf. Do not edit; regenerate this module.
use serde::{Deserialize, Serialize};
use crate::models;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ServerListingItem {
    ServerResponseWithIp(models::ServerResponseWithIp),
    ServerResponse(models::ServerResponse),
}

impl Default for ServerListingItem {
    fn default() -> Self {

        Self::ServerResponseWithIp(Default::default())

    }
}
