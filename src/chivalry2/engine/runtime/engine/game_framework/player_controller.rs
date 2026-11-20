//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/GameFramework/PlayerController.h>.

#![allow(c_void_returns)]
use crate::chivalry2::engine::FGuid;
use crate::chivalry2::engine::FName;
use crate::chivalry2::engine::FString;
use crate::chivalry2::engine::UObjectType;
use crate::chivalry2::engine::*;
use std::ffi::c_void;

define_pattern_resolver!(
    AuthenticateConsoleCommandCaller,
    ["74 ?? 40 32 FF E9 ?? ?? ?? ?? 49 8B 4D"]
);

define_pattern_resolver!(ClientMessage, [
    "4C 8B DC 48 83 EC 58 33 C0 49 89 5B 08 49 89 73 18 49 8B D8 49 89 43 C8 48 8B F1 49 89 43 D0 49 89 43 D8 49 8D 43"
]);

define_pattern_resolver!(
    ClientTravelInternal,
    ["4C 8B DC 48 83 EC 58 33 C0 49 89 5B 08 49 89 6B 10 41 0F B6 E9 49 89 73 18 48"]
);

/// Partial native controller layout through Player; later fields are omitted.
#[rust_ctables::cpp_struct]
#[derive(Debug)]
pub struct APlayerController {
    #[inherit]
    pub base: crate::chivalry2::engine::AController,
    pub player: *mut UPlayer, // 0x02D0
}

#[unchained_macros::runtime_bindings]
impl APlayerController {
    // void APlayerController::ClientMessage(const FString& S, FName Type, float MsgLifeTime)
    #[unchained_macros::bind_resolved_func(ClientMessage)]
    pub(crate) unsafe fn client_message(
        self: *mut Self,
        S: *mut FString,
        Type: FName,
        MsgLifeTime: f32,
    ) {
    }

    #[unchained_macros::bind_resolved_func(ClientTravelInternal)]
    pub(crate) unsafe fn client_travel_internal(
        self: *mut Self,
        url: *mut FString,
        travel_type: u8,
        seamless: bool,
        guid: *mut FGuid,
    ) -> c_void {
    }
}

unsafe impl UObjectType for APlayerController {
    const CLASS_NAME: &'static str = "PlayerController";
}
