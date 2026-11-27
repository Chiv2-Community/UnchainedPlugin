#![doc = include_str!("../README.md")]
#![doc = include_str!("../resolver-declarations.md")]
#![doc = include_str!("../patches.md")]
#![doc = include_str!("../detours.md")]

pub use backend::{startup_resolvers, OffsetProvider, PatternsleuthResolver, RequiredResolver, ScanContext};
pub use context::SelectorSnapshotExt;
pub use unchained_macros_patternsleuth_impl::define_pattern_resolver;

mod backend;
mod context;
mod custom_resolver;
mod signatures;

#[doc(hidden)]

pub mod __private {

    pub use crate::context::context_snapshot;
    pub use crate::custom_resolver::single_address;
    pub use unchained_macros_core::__private::*;
}
