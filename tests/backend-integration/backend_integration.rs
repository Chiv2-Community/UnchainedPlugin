//! Exercise the plugin ban-check bridge against the real backend.
#[path = "../../src/backend.rs"]
mod backend;
mod support;
use server_browser_client::{apis::api_v1_api as api, models::IpListRequest};

#[test_case::test_case(false; "allowed")]
#[test_case::test_case(true; "banned")]
fn native_ban_bridge_against_backend(banned: bool) {
    let (_container, url) = support::backend();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    if banned {
        runtime.block_on(async {
            let client = backend::transport(&url, None).unwrap();
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
        backend::check_banned_sync(&url, "127.0.0.1", runtime.handle()),
        Some(banned)
    );
}
