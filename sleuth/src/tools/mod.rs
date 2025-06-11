
pub mod syslog;
pub mod logger;
#[macro_use]
// pub mod log_macros_squashed;
// pub mod log_macros_kv;
pub mod log_macros;

#[cfg(feature="server-registration")]
pub mod server_registration;