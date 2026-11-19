//! Plugin replacements for `crate::chivalry2::engine::runtime::engine::kismet::gameplay_statics`.

use crate::chivalry2::engine::runtime::engine::kismet::gameplay_statics::*;
use crate::chivalry2::engine::FString;
use crate::chivalry2::engine::USaveGame;

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
