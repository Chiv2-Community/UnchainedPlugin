//! Dedicated-server status overrides for playable listen servers.

use crate::Chivalry2::Engine::{ENetMode, UObject};
use crate::Chivalry2::TBL::TBLTypes::UGameplay;

#[unchained_macros::bind_func_detour(UGameplay::IsDedicatedServer, || crate::tools::hook_globals::cli_args().playable_listen)]
pub(crate) unsafe fn is_dedicated_server_detour(context: *mut UObject) -> bool {
    if let Some(world) = crate::tools::hook_globals::globals().world() {
        let mode = unsafe { world.InternalGetNetMode() };
        if matches!(mode, ENetMode::DEDICATED_SERVER | ENetMode::LISTEN_SERVER) {
            #[cfg(feature = "verbose_hooks")]
            strace!("Overriding dedicated server status");
            return true;
        }
    }
    unsafe { UGameplay::IsDedicatedServer(context) }
}
