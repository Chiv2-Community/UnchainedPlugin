//! Hosting desync workarounds and Blueprint recursion protection.

pub mod net_mode;
pub mod prevent_stackoverflow;
#[cfg(feature = "move-autonomous-desync")]
pub mod tick_actors;
