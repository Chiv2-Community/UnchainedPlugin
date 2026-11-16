//! Exercise the pinned lifecycle SDK against the real containerized backend.
mod support;
#[path = "../../src/backend.rs"]
mod backend;
use server_browser_client::{
    apis::api_v1_api as api,
    models::{
        Chivalry2Ports, ServerListingItem, ServerRegistrationRequest, UpdateRegisteredServer,
    },
};
use server_browser_client_sdk::{Event, Policy, Registration};

#[test]
fn registration_lifecycle_against_backend_container() {
    let (_container, url) = support::backend();
    // Container setup and drop stay outside async workers.
    let runtime = tokio::runtime::Runtime::new().unwrap();
    assert_eq!(
        backend::check_banned_sync(&url, "127.0.0.1", runtime.handle()),
        Some(false)
    );
    runtime.block_on(async {
        let client = backend::transport(&url, None).unwrap();
        let config = client.configuration();
        let request = ServerRegistrationRequest::new(
            Chivalry2Ports::new(7777, 7778, 7779),
            "local-sdk-test".into(),
            "".into(),
            "map".into(),
            0,
            32,
        );
        let mut registration =
            Registration::new(client.clone(), request, Policy::default()).unwrap();
        assert!(matches!(
            registration.poll().await,
            Ok(Event::Registered { .. })
        ));
        registration
            .set_update(UpdateRegisteredServer::new(2, 32, "next".into()))
            .unwrap();
        assert_eq!(registration.poll().await, Ok(Event::Updated));
        let listing = api::get_servers(config)
            .await
            .unwrap_or_else(|_| panic!("listing failed"));
        let [ServerListingItem::ServerResponse(server)] = listing.servers.as_slice() else {
            panic!("expected one public listing");
        };
        assert_eq!(server.player_count, 2);
        assert_eq!(server.current_map, "next");
        registration.stop().await.unwrap();
        assert!(api::get_servers(config)
            .await
            .unwrap_or_else(|_| panic!("listing failed"))
            .servers
            .is_empty());
    });
}
