//! Exercise the pinned lifecycle SDK through plugin transport against a local backend.
use std::{
    cell::Cell,
    time::{Duration, Instant},
};
#[path = "../../src/backend.rs"]
mod backend;
use server_browser_client::{
    apis::api_v1_api as api,
    models::{Chivalry2Ports, ServerRegistrationRequest, UpdateRegisteredServer},
};
use server_browser_client_sdk::{Clock, Event, Policy, Registration};

struct TestClock(Cell<Instant>);
impl Clock for &TestClock {
    fn now(&self) -> Instant {
        self.0.get()
    }
}

fn main() {
    let url = std::env::args().nth(1).expect("local backend URL required");
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
    let clock = TestClock(Cell::new(Instant::now()));
    let mut registration =
        Registration::with_clock(client.clone(), request, Policy::default(), &clock).unwrap();
    assert!(matches!(registration.poll(), Ok(Event::Registered { .. })));
    assert!(api::get_servers(config).is_ok());
    registration
        .set_update(UpdateRegisteredServer::new(2, 32, "next".into()))
        .unwrap();
    assert_eq!(registration.poll(), Ok(Event::Updated));
    clock.0.set(clock.0.get() + Duration::from_secs(31));
    assert_eq!(registration.poll(), Ok(Event::Heartbeat));
    assert!(
        !api::check_banned(config, "127.0.0.1")
            .unwrap_or_else(|_| panic!("ban check failed"))
            .banned
    );
    assert!(api::jwks(config).is_ok());
    registration.stop().unwrap();
    assert!(api::get_servers(config)
        .unwrap_or_else(|_| panic!("listing failed"))
        .servers
        .is_empty());
    println!("Pinned SDK registration/update/heartbeat/cleanup and generated list/ban/JWKS operations passed");
}
