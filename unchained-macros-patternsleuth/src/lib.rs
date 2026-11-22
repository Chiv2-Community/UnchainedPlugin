#![doc = include_str!("../README.md")]
#![doc = include_str!("../resolver-declarations.md")]
#![doc = include_str!("../patches.md")]
#![doc = include_str!("../detours.md")]
pub use unchained_macros_patternsleuth_impl::define_pattern_resolver;
pub use context::SelectorSnapshotExt;
pub use backend::{PatternsleuthResolver, ScanContext, RequiredResolver, OffsetProvider, startup_resolvers};

mod backend;
mod context;
mod signatures;
mod custom_resolver;

#[doc(hidden)]
pub mod __private {
    pub use unchained_macros_core::__private::*;
    pub use crate::context::context_snapshot;
    pub use crate::custom_resolver::single_address;
}
