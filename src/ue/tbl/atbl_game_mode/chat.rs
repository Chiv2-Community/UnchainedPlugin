//! ArgonSDK main: [ATBLGameMode](https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/TBLGameMode.h).

use crate::game::chivalry2::AActor;
use crate::ue::engine::aplayer_controller::chat::client_message::{
    actor_player_state, capture_chat,
};
use crate::ue::{ATBLGameMode, FString, UObjectType};

define_pattern_resolver!(BroadcastChat, [
    "48 85 D2 0F 84 3E 04 00 00 48 8B C4 4C 89 40 18 48 89 48 08 55 41 55 41 56 41 57 48 8D 68 A9 48"
]);

#[unchained_macros::runtime_bindings]
impl ATBLGameMode {
    #[bind_resolved_func_detour(BroadcastChat)]
    unsafe fn broadcast_chat_detour(
        self: *mut Self,
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
            self.broadcast_chat(sender, message, chat_type, override_color);
        }
    }
}
