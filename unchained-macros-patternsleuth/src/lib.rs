#![doc = include_str!("../README.md")]
pub use unchained_macros_core::*;
pub use unchained_macros_patternsleuth_impl::{define_pattern_resolver, runtime_bindings, bind_resolved_request_function};
pub use context::SelectorSnapshotExt;
pub use backend::{PatternsleuthResolver, ScanContext, RequiredResolver, OffsetRegisty,
    startup_resolvers, initialize_function_bindings, initialize_function_bindings_where,
    apply_registered_patches};

mod backend;
mod context;
mod signatures;
mod custom_resolver;
pub type ResolverDescriptor = Resolver<PatternsleuthResolver>;

#[doc(hidden)]
pub mod __private {
    pub use unchained_macros_core::__private::*;
    pub use crate::context::context_snapshot;
    pub use crate::custom_resolver::single_address;
}
