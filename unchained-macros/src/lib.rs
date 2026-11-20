//! Hook, detour, patch and signature macros for the plugin.
mod detour;
pub use detour::{detour_is_initialized, detour_is_enabled};

pub use unchained_macros_impl::{
    bind_resolved_func, bind_resolver_value, runtime_bindings,
    bind_resolved_request_function,
    bind_func_detour,
};
pub use binding::{FunctionBinding, FunctionBindingRegistration, ValueBinding, ValueBindingRegistration};
#[doc(hidden)]
pub use binding::NativeFunction;
pub use binding::{initialize_value_binding, initialize_value_bindings, is_required_value};
pub use binding::{initialize_function_bindings, function_is_bound, require_function_bindings};
pub use registry::{
    CreateHookFn, HookRegistration, RegistrationPredicate, CreatePatchFn,
    PatchRegistration, OffsetRegisty,
};
pub use platforms::{PlatformType, PLATFORM, current_platform, ResolverPlatform, supports_platform};

mod binding;
mod registry;
mod platforms;
mod patch;
mod signatures;
pub mod signature;
mod custom_resolver;

#[doc(hidden)]
pub mod __private {
    pub use crate::detour::claim_detour_address;
    pub use crate::binding::publish_function_trampolines;
    pub use crate::custom_resolver::single_address;
    pub use log;
    pub use paste;
}
