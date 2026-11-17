#[path = "../../src/features/registration_ports.rs"]
mod registration_ports;

use test_case::test_case;

#[test_case(Some(8000), Some(9000), Some(3000), 7000, [8000, 3000, 7000]; "a2s_port_wins")]
#[test_case(None, Some(9000), Some(3000), 7000, [9000, 3000, 7000]; "configured_port_fallback")]
#[test_case(None, None, None, 7071, [7777, 0, 7071]; "defaults")]
#[test_case(Some(0), Some(9000), Some(0), 0, [0, 0, 0]; "explicit_zero_preserved")]
fn advertised_ports_use_a2s_then_configuration_then_defaults(
    a2s_game_port: Option<u16>,
    configured_game_port: Option<u16>,
    ping_port: Option<u16>,
    query_port: u16,
    expected: [i32; 3],
) {
    let ports = registration_ports::resolve(
        a2s_game_port,
        configured_game_port,
        ping_port,
        query_port,
    );
    assert_eq!([ports.game, ports.ping, ports.a2s], expected);
}
