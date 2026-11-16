use std::net::TcpListener;

#[path = "../../src/backend.rs"]
mod backend;

#[test]
fn transport_sets_plugin_user_agent() {
    let client = backend::transport("http://127.0.0.1:18080", None).unwrap();
    assert_eq!(
        client.configuration().user_agent.as_deref(),
        Some("UnchainedPlugin/server-browser-api")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn native_ban_bridge_fails_open_inside_runtime_without_deadlock() {
    assert_eq!(
        backend::check_banned_sync(
            &backend::transport("http://127.0.0.1:1", None).unwrap(),
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
        backend::check_banned_sync(
            &backend::transport(&url, None).unwrap(),
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
        backend::check_banned_sync(
            &backend::transport("http://127.0.0.1:1", None).unwrap(),
            "127.0.0.1",
            &handle
        ),
        None
    );
}
