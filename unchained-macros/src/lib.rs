//! Hook, detour, patch and signature macros for the plugin.

pub use unchained_macros_impl::{bind_resolver_func, bind_resolver_value};
pub use binding::{FunctionBinding, FunctionBindingRegistration, ValueBinding, ValueBindingRegistration};
pub use binding::{initialize_value_binding, initialize_value_bindings, is_required_value};

mod binding;
mod patch;
mod hooks;
mod signatures;
