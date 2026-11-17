#[path = "../../src/features/registration_ports.rs"]
mod registration_ports;

#[test]
fn advertised_ports_use_a2s_then_configuration_then_defaults() {
    for (reported, configured, ping, query, expected) in [
        (Some(8000), Some(9000), Some(3000), 7000, [8000, 3000, 7000]),
        (None, Some(9000), Some(3000), 7000, [9000, 3000, 7000]),
        (None, None, None, 7071, [7777, 0, 7071]),
        (Some(0), Some(9000), Some(0), 0, [0, 0, 0]),
    ] {
        let ports = registration_ports::resolve(reported, configured, ping, query);
        assert_eq!([ports.game, ports.ping, ports.a2s], expected);
    }
}
