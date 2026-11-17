//! Hook, detour, patch and signature macros for the plugin.

pub use unchained_macros_impl::bind_resolver_func;
pub use binding::{FunctionBinding, FunctionBindingRegistration};

mod binding;
mod patch;
mod hooks;
mod signatures;
