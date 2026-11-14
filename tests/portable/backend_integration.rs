//! Exercise the pinned lifecycle SDK against the real containerized backend.
mod support;
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

#[test]
fn backend_signed_ticket_verifies_once_for_live_registration() {
    use testcontainers::core::ExecCommand;
    use unchained_auth::{AuthError, TicketVerifier};

    let (container, url) = support::backend();
    let client = backend::transport(&url, None).unwrap();
    let request = ServerRegistrationRequest::new(
        Chivalry2Ports::new(7777, 7778, 7779),
        "ticket-test".into(),
        "".into(),
        "map".into(),
        0,
        32,
    );
    let mut registration = Registration::new(client.clone(), request, Policy::default()).unwrap();
    assert!(matches!(registration.poll(), Ok(Event::Registered { .. })));
    let id = registration.server_id().unwrap();
    // Exercise the backend's internal signing boundary inside its container.
    // No test-only HTTP endpoint or private key crosses into the plugin.
    let mut issued = container.exec(ExecCommand::new([
        "python", "-c",
        "import json,sys,time; from pathlib import Path; from server_browser_backend.auth.signing import SigningKeys; now=int(time.time()); ticket=SigningKeys(Path('/tmp/browser-test/signing.json')).issue_join_ticket(issuer='https://local-auth.test',discord_id='123456789012345678',server_id=sys.argv[1],auth_time=now); print(json.dumps({'ticket':ticket,'now':now}))",
        id,
    ])).expect("issue ticket inside backend container");
    let output = issued.stdout_to_vec().unwrap();
    assert_eq!(issued.exit_code().unwrap(), Some(0));
    let bundle: serde_json::Value = serde_json::from_slice(&output).expect("decode signing result");
    let ticket = bundle["ticket"].as_str().unwrap();
    let now = bundle["now"].as_u64().unwrap();
    let jwks =
        api::jwks(client.configuration()).unwrap_or_else(|_| panic!("JWKS retrieval failed"));
    let jwks = serde_json::to_vec(&jwks).unwrap();
    let mut verifier = TicketVerifier::new("https://local-auth.test", id, &jwks, 32).unwrap();
    let identity = verifier.verify_and_consume(ticket, now).unwrap();
    assert_eq!(identity.discord_id(), "123456789012345678");
    assert_eq!(identity.server_id().to_string(), id);
    assert_eq!(
        verifier.verify_and_consume(ticket, now),
        Err(AuthError::Replay)
    );
    let mut wrong_server = TicketVerifier::new(
        "https://local-auth.test",
        "00000000-0000-0000-0000-000000000001",
        &jwks,
        32,
    )
    .unwrap();
    assert!(wrong_server.verify_and_consume(ticket, now).is_err());
    let mut logs = container.stdout_to_vec().unwrap();
    logs.extend(container.stderr_to_vec().unwrap());
    assert!(
        !logs
            .windows(ticket.len())
            .any(|window| window == ticket.as_bytes()),
        "ticket leaked to backend logs"
    );
    registration.stop().unwrap();
}

#[test]
fn registration_lifecycle_against_backend_container() {
    let (_container, url) = support::backend();
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
    let listing = api::get_servers(config).unwrap_or_else(|_| panic!("listing failed"));
    assert_eq!(listing.servers.len(), 1);
    registration
        .set_update(UpdateRegisteredServer::new(2, 32, "next".into()))
        .unwrap();
    assert_eq!(registration.poll(), Ok(Event::Updated));
    let listing = api::get_servers(config).unwrap_or_else(|_| panic!("listing failed"));
    let server = serde_json::to_value(&listing.servers[0]).unwrap();
    assert_eq!(server["player_count"], 2);
    assert_eq!(server["current_map"], "next");
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
}
