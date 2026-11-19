//! Plugin replacements for `crate::chivalry2::engine::engine::kismet::gameplay_statics`.

use crate::chivalry2::engine::engine::kismet::gameplay_statics::*;
use crate::chivalry2::engine::FString;
use crate::chivalry2::engine::USaveGame;

#[unchained_macros::bind_func_detour(UGameplayStatics::is_dedicated_server, || crate::tools::hook_globals::cli_args().playable_listen)]
pub(crate) unsafe fn is_dedicated_server_detour(context: u64) -> bool {
    use crate::chivalry2::engine::ENetMode;
    if let Some(world) = crate::tools::hook_globals::globals().world() {
        let mode = unsafe { world.internal_get_net_mode() };
        if matches!(mode, ENetMode::DEDICATED_SERVER | ENetMode::LISTEN_SERVER) {
            #[cfg(feature = "verbose_hooks")]
            strace!("Overriding dedicated server status");
            return true;
        }
    }
    unsafe { UGameplayStatics::is_dedicated_server(context) }
}

#[unchained_macros::bind_func_detour(UGameplayStatics::load_game_from_slot)]
pub(crate) unsafe fn load_game_from_slot_detour<'a>(
    slot: &'a FString,
    index: i32,
) -> *mut USaveGame {
    sdebug!("Triggered! {}", slot);
    unsafe { UGameplayStatics::load_game_from_slot(slot, index) }
}

#[unchained_macros::bind_func_detour(UGameplayStatics::save_game_to_slot)]
pub(crate) unsafe fn save_game_to_slot_detour<'a>(
    game: *mut USaveGame,
    slot: &'a FString,
    index: i32,
) -> bool {
    sdebug!("Triggered! {}", slot);
    unsafe { UGameplayStatics::save_game_to_slot(game, slot, index) }
}
