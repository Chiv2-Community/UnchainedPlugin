//! Native source: <https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/EChatType.h>.

#![allow(non_snake_case)]
use crate::Chivalry2::Engine::*;
use crate::Chivalry2::TBL::EChatType::EChatType;
use tokio_util::sync::CancellationToken;

// Helper functions
pub(crate) type UWorldAccessor = fn() -> Option<*mut UWorld>;

pub fn send_ingame_message(message: String, chat_type: Option<EChatType>) {
    send_ingame_message_with_world(message, chat_type, || crate::globals().world(), None);
}

pub(crate) fn send_ingame_message_with_world(
    message: String,
    chat_type: Option<EChatType>,
    world_accessor: UWorldAccessor,
    cancellation: Option<CancellationToken>,
) {
    use crate::Chivalry2::TBL::UTBLSystemLibrary;
    use crate::unchained::patches::unchained_integration::game_thread::run_on_game_thread;

    // Cancellation, world resolution and native work must share one game-thread job.
    run_on_game_thread(move || {
        if cancellation
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
        {
            return;
        }
        let Some(world) = world_accessor().filter(|world| !world.is_null()) else {
            return;
        };
        let game_mode = unsafe { UTBLSystemLibrary::GetTBLGameMode(world.as_uobject()) };
        if game_mode.is_null() {
            return;
        }
        let chat_type_actual = chat_type.unwrap_or(EChatType::AllSay);
        let text = crate::Chivalry2::Engine::InvariantText::new(&message);
        unsafe {
            game_mode.BroadcastLocalizedChat(text.as_ptr(), chat_type_actual);
        }
    });
}
