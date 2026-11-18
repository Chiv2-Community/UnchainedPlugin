use crate::features::Mod;
use server_browser_client::models::{
    Chivalry2Ports, Mod as BrowserMod, ServerRegistrationRequest, UpdateRegisteredServer,
};

#[derive(Clone, Debug)]
pub struct ServerStatus {
    pub name: String,
    pub description: String,
    pub password_protected: bool,
    pub current_map: String,
    pub player_count: i32,
    pub max_players: i32,
    pub all_mods: Vec<Mod>,
    pub active_mods: Vec<Mod>,
}

impl ServerStatus {
    pub(super) fn registration_request(
        &self,
        ports: Chivalry2Ports,
        local_ip_address: Option<String>,
    ) -> ServerRegistrationRequest {
        let update = self.update_request();
        ServerRegistrationRequest {
            ports,
            name: self.name.clone(),
            description: self.description.clone(),
            password_protected: Some(self.password_protected),
            current_map: update.current_map,
            player_count: update.player_count,
            max_players: update.max_players,
            local_ip_address,
            mods: update.mods,
        }
    }

    pub(super) fn update_request(&self) -> UpdateRegisteredServer {
        UpdateRegisteredServer {
            player_count: self.player_count,
            max_players: self.max_players,
            current_map: self.current_map.clone(),
            mods: Some(
                self.active_mods
                    .iter()
                    .map(|metadata| {
                        BrowserMod::new(
                            metadata.name.clone(),
                            metadata.organization.clone(),
                            metadata.version.clone(),
                        )
                    })
                    .collect(),
            ),
        }
    }
}

pub(super) fn ports(
    a2s_game_port: Option<u16>,
    configured_game_port: u16,
    ping_port: u16,
    query_port: u16,
) -> Chivalry2Ports {
    Chivalry2Ports {
        game: i32::from(a2s_game_port.unwrap_or(configured_game_port)),
        ping: i32::from(ping_port),
        a2s: i32::from(query_port),
    }
}

pub(super) fn player_counts(players: u8, max_players: u8, headless: bool) -> (i32, i32) {
    // A2S counts the headless host as a player and an occupied slot.
    let host = u8::from(headless);
    (
        i32::from(players.saturating_sub(host)),
        i32::from(max_players.saturating_sub(host)),
    )
}
