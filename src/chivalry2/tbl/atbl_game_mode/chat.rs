//! ArgonSDK main: [ATBLGameMode](https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/TBLGameMode.h).

use crate::chivalry2::engine::AActor;
use crate::chivalry2::engine::FString;
use crate::chivalry2::tbl::ATBLGameMode;

define_pattern_resolver!(BroadcastChat, [
    "48 85 D2 0F 84 3E 04 00 00 48 8B C4 4C 89 40 18 48 89 48 08 55 41 55 41 56 41 57 48 8D 68 A9 48"
]);

#[unchained_macros::runtime_bindings]
impl ATBLGameMode {
    #[unchained_macros::bind_resolved_func(BroadcastChat)]
    pub(crate) unsafe fn broadcast_chat(
        self: *mut Self,
        sender: *mut AActor,
        message: *const FString,
        chat_type: u32,
        override_color: u32,
    ) {
    }
}
