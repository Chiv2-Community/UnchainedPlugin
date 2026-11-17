use server_browser_client::models::Chivalry2Ports;

pub(super) fn resolve(
    a2s_game_port: Option<u16>,
    configured_game_port: Option<u16>,
    ping_port: Option<u16>,
    query_port: u16,
) -> Chivalry2Ports {
    Chivalry2Ports {
        game: i32::from(a2s_game_port.or(configured_game_port).unwrap_or(7777)),
        ping: i32::from(ping_port.unwrap_or(0)),
        a2s: i32::from(query_port),
    }
}
