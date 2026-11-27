#[cfg(feature = "server_registration")]
pub(crate) mod http;
#[cfg(feature = "server_registration")]
pub(crate) mod server_metadata;
pub(crate) mod urls;
#[cfg(feature = "server_registration")]
mod a2s_query;
#[cfg(feature = "server_registration")]
pub(crate) mod registration;
