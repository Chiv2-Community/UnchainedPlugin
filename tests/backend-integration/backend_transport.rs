use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    thread,
};

#[path = "../../src/features/a2s_query.rs"]
mod a2s_query;
mod a2s_transport;
#[path = "../../src/backend.rs"]
mod backend;
#[path = "../../src/features/registration_control.rs"]
mod registration_control;
use server_browser_client::{
    apis::api_v1_api,
    models::{Chivalry2Ports, ServerRegistrationRequest},
};
use server_browser_client_sdk::{Policy, Registration, SdkError};

fn serve(response: String) -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let worker = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        let mut stream = BufReader::new(stream);
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            assert_ne!(
                stream.read_until(b'\n', &mut request).unwrap(),
                0,
                "incomplete HTTP headers"
            );
        }
        let length = String::from_utf8_lossy(&request)
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
            .map(|(_, value)| value.trim().parse::<usize>().unwrap())
            .unwrap_or(0);
        let mut body = vec![0; length];
        stream.read_exact(&mut body).unwrap();
        request.extend(body);
        stream.get_mut().write_all(response.as_bytes()).unwrap();
        String::from_utf8(request).unwrap()
    });
    (url, worker)
}

fn serve_json(body: serde_json::Value) -> (String, thread::JoinHandle<String>) {
    let body = body.to_string();
    serve(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()))
}

#[test_case::test_case("127.0.0.1", "127.0.0.1", true; "ipv4_banned")]
#[test_case::test_case("2001:db8::1", "2001%3Adb8%3A%3A1", false; "ipv6_allowed")]
#[tokio::test]
async fn generated_ban_check_uses_typed_boolean_and_encoded_ip(
    address: &str,
    encoded: &str,
    banned: bool,
) {
    let (url, worker) =
        serve_json(serde_json::json!({"banned": banned, "note": (!banned).to_string()}));
    let client = backend::transport(&url, None).unwrap();
    assert_eq!(
        api_v1_api::check_banned(client.configuration(), address)
            .await
            .unwrap()
            .banned,
        banned
    );
    let request = worker.join().unwrap();
    assert!(
        request.starts_with(&format!("GET /api/v1/check-banned/{encoded} ")),
        "{request}"
    );
}

#[tokio::test]
async fn credentials_never_follow_redirects_and_debug_redacts_url() {
    let (url, worker) = serve("HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/stolen\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into());
    let client = backend::transport(&url, None).unwrap();
    assert!(
        api_v1_api::delete_server(client.configuration(), "credential-sentinel", "id")
            .await
            .is_err()
    );
    let request = worker.join().unwrap().to_lowercase();
    assert!(request.starts_with("delete /api/v1/servers/id "));
    assert!(request.contains("x-chiv2-server-browser-key: credential-sentinel"));
    assert_eq!(format!("{client:?}"), "HttpTransport([REDACTED])");
}

#[tokio::test]
async fn sdk_transport_uses_plugin_policy_and_halts_ambiguous_registration() {
    let (url, worker) = serve("HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/stolen\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into());
    let client = backend::transport(&url, None).unwrap();
    let request = ServerRegistrationRequest::new(
        Chivalry2Ports::new(7777, 7778, 7779),
        "server".into(),
        "test".into(),
        "map".into(),
        0,
        32,
    );
    let mut registration = Registration::new(client, request, Policy::default()).unwrap();
    assert_eq!(
        registration.poll().await,
        Err(SdkError::RegistrationUncertain)
    );
    assert_eq!(
        registration.poll().await,
        Err(SdkError::RegistrationUncertain)
    );
    let request = worker.join().unwrap();
    assert!(request.starts_with("POST /api/v1/servers "));
    assert!(request
        .to_lowercase()
        .contains("user-agent: unchainedplugin/server-browser-api"));
    let (_, body) = request.split_once("\r\n\r\n").unwrap();
    let body: ServerRegistrationRequest = serde_json::from_str(body).unwrap();
    assert_eq!(body.name, "server");
    assert_eq!(body.current_map, "map");
}

#[test_case::test_case(false; "allowed")]
#[test_case::test_case(true; "banned")]
fn native_ban_bridge_awaits_generated_operation_on_shared_runtime(banned: bool) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (url, worker) = serve_json(serde_json::json!({"banned": banned}));
    assert_eq!(
        backend::check_banned_sync(&url, "127.0.0.1", runtime.handle()),
        Some(banned)
    );
    assert!(worker
        .join()
        .unwrap()
        .starts_with("GET /api/v1/check-banned/127.0.0.1 "));
}

#[tokio::test(flavor = "current_thread")]
async fn native_ban_bridge_fails_open_inside_runtime_without_deadlock() {
    assert_eq!(
        backend::check_banned_sync(
            "http://127.0.0.1:1",
            "127.0.0.1",
            &tokio::runtime::Handle::current()
        ),
        None
    );
}

#[test]
fn native_ban_bridge_fails_open_on_connection_error() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let runtime = tokio::runtime::Runtime::new().unwrap();
    assert_eq!(
        backend::check_banned_sync(&url, "127.0.0.1", runtime.handle()),
        None
    );
}

#[test]
fn native_ban_bridge_fails_open_after_runtime_shutdown() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let handle = runtime.handle().clone();
    drop(runtime);
    assert_eq!(
        backend::check_banned_sync("http://127.0.0.1:1", "127.0.0.1", &handle),
        None
    );
}

#[test_case::test_case(false; "plain")]
#[test_case::test_case(true; "diagnostic")]
fn generated_listing_union_preserves_plain_and_diagnostic_shapes(diagnostic: bool) {
    let mut value = serde_json::json!({
        "unique_id": "id", "ports": {"game": 7777, "ping": 7778, "a2s": 7779},
        "password_protected": false, "is_verified": false, "name": "test", "description": "",
        "current_map": "map", "player_count": 0, "max_players": 32, "mods": [], "last_heartbeat": 1800000000.0
    });
    if diagnostic {
        value["ip_address"] = serde_json::json!("2001:db8::1");
        value["local_ip_address"] = serde_json::Value::Null;
    }
    let parsed: server_browser_client::models::ServerListingItem =
        serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), value);
}
