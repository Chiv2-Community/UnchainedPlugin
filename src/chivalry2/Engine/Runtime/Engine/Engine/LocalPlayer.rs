//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/Engine/LocalPlayer.h>.

use crate::chivalry2::Engine::*;

#[repr(C)]
pub struct ULocalPlayer {
    pub base: UPlayer,
}

unsafe impl UObjectType for ULocalPlayer {
    const CLASS_NAME: &'static str = "LocalPlayer";
}
