pub mod http;
pub mod server_metadata;
pub mod urls;
#[cfg(feature = "server_registration")]
mod a2s_query;
#[cfg(feature = "server_registration")]
pub mod registration;
