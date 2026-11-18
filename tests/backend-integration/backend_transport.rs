use std::net::TcpListener;

#[path = "../../src/features/server_browser/http.rs"]
mod http;

#[path = "../../src/features/server_browser/urls.rs"]
mod urls;

#[test_case::test_case("https://example.test", "https://example.test"; "root")]
#[test_case::test_case("https://example.test/", "https://example.test"; "trailing_slash")]
#[test_case::test_case("https://example.test/browser///", "https://example.test/browser"; "deployment_prefix")]
fn native_compatibility_urls(base: &str, prefix: &str) {
    assert_eq!(urls::tbio_base_url(base), format!("{prefix}/api/tbio"));
    assert_eq!(urls::playfab_matchmake_url(base), format!("{prefix}/api/playfab/Client/Matchmake"));
}

#[tokio::test(flavor = "current_thread")]
async fn native_ban_bridge_fails_open_inside_runtime_without_deadlock() {
    assert_eq!(
        http::check_banned_sync(
            &server_browser_client_sdk::HttpTransport::new("http://127.0.0.1:1", None).unwrap(),
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
        http::check_banned_sync(
            &server_browser_client_sdk::HttpTransport::new(&url, None).unwrap(),
            "127.0.0.1",
            runtime.handle()
        ),
        None
    );
}

#[test]
fn native_ban_bridge_fails_open_after_runtime_shutdown() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let handle = runtime.handle().clone();
    drop(runtime);
    assert_eq!(
        http::check_banned_sync(
            &server_browser_client_sdk::HttpTransport::new("http://127.0.0.1:1", None).unwrap(),
            "127.0.0.1",
            &handle
        ),
        None
    );
}
