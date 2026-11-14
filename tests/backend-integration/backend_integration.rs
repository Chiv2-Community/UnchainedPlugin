//! Exercise the pinned lifecycle SDK against the real containerized backend.
mod support;
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
#[path = "../../src/backend.rs"]
mod backend;
use server_browser_client::{
    apis::api_v1_api as api,
    models::{Chivalry2Ports, ServerRegistrationRequest, UpdateRegisteredServer},
};
use server_browser_client_sdk::{Clock, Event, Policy, Registration};

struct TestClock(Mutex<Instant>);
impl Clock for &TestClock {
    fn now(&self) -> Instant {
        *self.0.lock().unwrap()
    }
}

#[test]
fn registration_lifecycle_against_backend_container() {
    let (_container, url) = support::backend();
    // Docker/archive setup and Container drop stay outside async workers.
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
        let clock = TestClock(Mutex::new(Instant::now()));
        let mut registration =
            Registration::with_clock(client.clone(), request, Policy::default(), &clock).unwrap();
        assert!(matches!(
            registration.poll().await,
            Ok(Event::Registered { .. })
        ));
        let listing = api::get_servers(config)
            .await
            .unwrap_or_else(|_| panic!("listing failed"));
        assert_eq!(listing.servers.len(), 1);
        registration
            .set_update(UpdateRegisteredServer::new(2, 32, "next".into()))
            .unwrap();
        assert_eq!(registration.poll().await, Ok(Event::Updated));
        let listing = api::get_servers(config)
            .await
            .unwrap_or_else(|_| panic!("listing failed"));
        let server = serde_json::to_value(&listing.servers[0]).unwrap();
        assert_eq!(server["player_count"], 2);
        assert_eq!(server["current_map"], "next");
        *clock.0.lock().unwrap() += Duration::from_secs(31);
        assert_eq!(registration.poll().await, Ok(Event::Heartbeat));
        assert!(
            !api::check_banned(config, "127.0.0.1")
                .await
                .unwrap_or_else(|_| panic!("ban check failed"))
                .banned
        );
        assert!(api::jwks(config).await.is_ok());
        registration.stop().await.unwrap();
        assert!(api::get_servers(config)
            .await
            .unwrap_or_else(|_| panic!("listing failed"))
            .servers
            .is_empty());
    });
}
