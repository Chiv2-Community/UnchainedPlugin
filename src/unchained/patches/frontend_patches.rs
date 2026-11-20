//! Frontend startup options, mod asset initialization, and startup messages.

use crate::chivalry2::tbl::tbl_game_instance::*;
use crate::tools::hook_globals::cli_args;
use std::sync::atomic::{AtomicBool, Ordering};

#[unchained_macros::bind_func_detour(UTBLGameInstance::load_front_end_map)]
pub(crate) unsafe fn load_front_end_map_detour(
    this: *mut UTBLGameInstance,
    _url: *mut crate::chivalry2::engine::FString,
) -> bool {
    static INITIALIZED: AtomicBool = AtomicBool::new(false);
    let args = cli_args();
    let password = args
        .server_password
        .as_ref()
        .map(|value| format!("?Password={value}"))
        .unwrap_or_default();
    let rcon = if args.rcon_port.is_some() {
        "?rcon"
    } else {
        ""
    };
    let first_load = INITIALIZED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok();
    let startup = if first_load { "?startup" } else { "" };
    if first_load {
        // Mounting precedes frontend loading; registration must not unwind
        // through the native callback.
        let result = std::panic::catch_unwind(|| {
            crate::features::mod_maps::auto_register_all_assets();
        });
        if result.is_err() {
            serror!("Mod asset registration panicked; continuing without it");
        }
    }
    let map_url = format!("Frontend{password}{rcon}{startup}");
    #[cfg(feature = "verbose_hooks")]
    sinfo!("{}", map_url);
    let mut map_url = crate::chivalry2::engine::FString::from(map_url.as_str());
    unsafe { this.load_front_end_map(&mut map_url) }
}

// Startup anti-cheat-message patch in UTBLGameInstance::Init.
CREATE_PATCH!(EACAntiCheatMesssage, 0xE, NOP, 5);
