//! UE 4.25-plus: [APlayerController](https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/GameFramework/PlayerController.h).

use crate::ue::APlayerController;
pub(crate) mod chat;
mod travel;

define_pattern_resolver!(DedicatedServerInit, [
    "41 56 48 83 EC 30 48 89 6C 24 48 48 89 74 24 50 48 89 7C 24 28 4C 89 7C 24 20 4C 8B F9 ?? ?? ?? ?? ?? 48 8B C8 4C 8B F0"
]);

#[unchained_macros::runtime_bindings]
impl APlayerController {
    #[bind_resolved_func_detour(DedicatedServerInit)]
    pub(crate) unsafe fn dedicated_server_init_detour(self: *mut Self) {}
}
