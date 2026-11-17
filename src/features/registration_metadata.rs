use crate::server_metadata::ServerStatus;
use server_browser_client::models::{
    Chivalry2Ports, Mod, ServerRegistrationRequest, UpdateRegisteredServer,
};

pub(super) fn request(
    status: &ServerStatus,
    ports: Chivalry2Ports,
    local_ip_address: Option<String>,
) -> ServerRegistrationRequest {
    let update = update(status);
    ServerRegistrationRequest {
        ports,
        name: status.name.clone(),
        description: status.description.clone(),
        password_protected: Some(status.password_protected),
        current_map: update.current_map,
        player_count: update.player_count,
        max_players: update.max_players,
        local_ip_address,
        mods: update.mods,
    }
}

pub(super) fn update(status: &ServerStatus) -> UpdateRegisteredServer {
    UpdateRegisteredServer {
        player_count: status.player_count,
        max_players: status.max_players,
        current_map: status.current_map.clone(),
        mods: Some(
            status
                .active_mods
                .iter()
                .map(|metadata| {
                    Mod::new(
                        metadata.name.clone(),
                        metadata.organization.clone(),
                        metadata.version.clone(),
                    )
                })
                .collect(),
        ),
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
