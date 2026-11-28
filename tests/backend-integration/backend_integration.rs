//! Exercise the plugin ban-check bridge against the real backend.
#[path = "../../unchained-plugin/src/features/server_browser/http.rs"]
mod http;
mod support;
use server_browser_client::{apis::api_v1_api as api, models::IpListRequest};

#[test_case::test_case(false; "allowed")]
#[test_case::test_case(true; "banned")]
fn native_ban_bridge_against_backend(banned: bool) {
    let (_container, url) = support::backend();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let client = server_browser_client_sdk::HttpTransport::new(&url, None).unwrap();
    if banned {
        runtime.block_on(async {
            api::add_to_ban_list(
                client.configuration(),
                "container-test-admin",
                IpListRequest {
                    banned_ips: Some(vec!["127.0.0.1".into()]),
                    verified_ips: None,
                },
            )
            .await
            .unwrap_or_else(|_| panic!("ban setup failed"));
        });
    }
    assert_eq!(
        http::check_banned_sync(&client, "127.0.0.1", runtime.handle()),
        Some(banned)
    );
}
