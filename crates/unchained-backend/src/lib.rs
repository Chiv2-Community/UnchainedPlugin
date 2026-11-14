//! Transport policy for the generated SDK; no duplicate routes or wire models.
use std::{net::IpAddr, time::Duration};

use apis::configuration::Configuration;
pub use server_browser_client::{apis, models};

#[derive(Clone)]
pub struct BackendClient(Configuration);

impl std::fmt::Debug for BackendClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BackendClient([REDACTED])")
    }
}

impl BackendClient {
    pub fn new(base_url: &str, source_ip: Option<IpAddr>) -> Result<Self, reqwest::Error> {
        let mut builder = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none());
        if let Some(ip) = source_ip {
            builder = builder.local_address(ip);
        }
        Ok(Self(Configuration {
            base_path: base_url.trim_end_matches('/').to_owned(),
            client: builder.build()?,
            user_agent: Some("UnchainedPlugin/server-browser-api".into()),
            basic_auth: None,
            oauth_access_token: None,
            bearer_access_token: None,
            api_key: None,
        }))
    }

    pub fn configuration(&self) -> &Configuration {
        &self.0
    }
}

pub fn invalidates_registration<E>(error: &apis::Error<E>) -> bool {
    matches!(error, apis::Error::ResponseError(response) if matches!(response.status.as_u16(), 401 | 403 | 404))
}
