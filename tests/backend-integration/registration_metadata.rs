#[path = "../../src/features/registration_metadata.rs"]
mod registration_metadata;

use test_case::test_case;

#[test_case(Some(8000), 9000, 3000, 7000, [8000, 3000, 7000]; "a2s_port_wins")]
#[test_case(None, 9000, 3000, 7000, [9000, 3000, 7000]; "configured_port_fallback")]
#[test_case(None, 7777, 3075, 7071, [7777, 3075, 7071]; "cli_defaults_preserved")]
#[test_case(Some(0), 9000, 0, 0, [0, 0, 0]; "explicit_zero_preserved")]
fn advertised_ports_prefer_a2s_game_port_over_configuration(
    a2s_game_port: Option<u16>,
    configured_game_port: u16,
    ping_port: u16,
    query_port: u16,
    expected: [i32; 3],
) {
    let ports =
        registration_metadata::ports(a2s_game_port, configured_game_port, ping_port, query_port);
    assert_eq!([ports.game, ports.ping, ports.a2s], expected);
}

#[test_case(12, 64, false, (12, 64); "normal_host_preserves_counts")]
#[test_case(12, 64, true, (11, 63); "headless_host_excluded")]
#[test_case(1, 64, true, (0, 63); "headless_host_only")]
#[test_case(0, 0, true, (0, 0); "empty_headless_response_does_not_underflow")]
#[test_case(0, 0, false, (0, 0); "empty_normal_response")]
#[test_case(255, 255, false, (255, 255); "full_a2s_range_preserved")]
fn advertised_player_counts(players: u8, max_players: u8, headless: bool, expected: (i32, i32)) {
    assert_eq!(
        registration_metadata::player_counts(players, max_players, headless),
        expected,
    );
}
