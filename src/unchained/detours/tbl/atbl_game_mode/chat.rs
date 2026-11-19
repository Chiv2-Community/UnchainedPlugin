//! Plugin replacements for `crate::chivalry2::tbl::atbl_game_mode::chat`.

use crate::chivalry2::engine::AActor;
use crate::chivalry2::engine::FString;
use crate::chivalry2::engine::UObjectType;
use crate::chivalry2::tbl::ATBLGameMode;
use crate::unchained::detours::engine::engine::game_framework::player_controller::{
    actor_player_state, capture_chat,
};

#[unchained_macros::bind_func_detour(ATBLGameMode::broadcast_chat)]
pub(crate) unsafe fn broadcast_chat_detour(
    this: *mut ATBLGameMode,
    sender: *mut AActor,
    message: *const FString,
    chat_type: u32,
    override_color: u32,
) {
    if let Some(player) =
        unsafe { AActor::from_ptr(sender).and_then(|actor| actor_player_state(actor)) }
    {
        capture_chat(player, message, chat_type);
    }
    unsafe {
        this.broadcast_chat(sender, message, chat_type, override_color);
    }
}
