//! Plugin-specific HTTP configuration; lifecycle policy belongs to the SDK.
use server_browser_client::apis::configuration::Configuration;
use server_browser_client_sdk::HttpTransport;
use std::net::IpAddr;

pub fn transport(
    base_url: &str,
    source_ip: Option<IpAddr>,
) -> Result<HttpTransport, reqwest::Error> {
    let transport = HttpTransport::new(base_url, source_ip)?;
    Ok(HttpTransport::from_configuration(Configuration {
        user_agent: Some("UnchainedPlugin/server-browser-api".into()),
        ..transport.configuration().clone()
    }))
}
