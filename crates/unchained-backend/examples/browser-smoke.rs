//! Exercise generated operations against a backend hosted on this machine.
use unchained_backend::{
    apis::api_v1_api as api,
    models::{Chivalry2Ports, ServerRegistrationRequest, UpdateRegisteredServer},
    BackendClient,
};

fn main() {
    let url = std::env::args().nth(1).expect("local backend URL required");
    let client = BackendClient::new(&url, None).unwrap();
    let config = client.configuration();
    let request = ServerRegistrationRequest::new(
        Chivalry2Ports::new(7777, 7778, 7779),
        "local-sdk-test".into(),
        "".into(),
        "map".into(),
        0,
        32,
    );
    // Do not print generated errors: they may retain credential-bearing response bodies.
    let registration =
        api::register(config, request).unwrap_or_else(|_| panic!("registration failed"));
    let id = &registration.server.unique_id;
    let key = &registration.key;
    assert!(api::get_servers(config).is_ok());
    assert!(api::update(
        config,
        key,
        id,
        UpdateRegisteredServer::new(2, 32, "next".into())
    )
    .is_ok());
    assert!(api::heartbeat(config, key, id).is_ok());
    assert!(
        !api::check_banned(config, "127.0.0.1")
            .unwrap_or_else(|_| panic!("ban check failed"))
            .banned
    );
    assert!(api::jwks(config).is_ok());
    assert!(api::delete_server(config, key, id).is_ok());
    println!("Generated registration/list/update/heartbeat/ban/JWKS/delete operations passed");
}
