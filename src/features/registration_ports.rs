use server_browser_client::models::Chivalry2Ports;

pub(super) fn resolve(
    reported_game: Option<u16>,
    configured_game: Option<u16>,
    configured_ping: Option<u16>,
    query: u16,
) -> Chivalry2Ports {
    Chivalry2Ports {
        game: i32::from(reported_game.or(configured_game).unwrap_or(7777)),
        ping: i32::from(configured_ping.unwrap_or(0)),
        a2s: i32::from(query),
    }
}
