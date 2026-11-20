//! Hosting desync workarounds and Blueprint recursion protection.

#[cfg(feature = "move-autonomous-desync")]
pub mod legacy_tick_actor;
#[cfg(feature = "move-autonomous-desync")]
pub mod movement;
pub mod net_mode;
pub mod prevent_stackoverflow;
