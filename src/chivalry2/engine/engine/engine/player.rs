//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/Engine/Player.h>.

use crate::chivalry2::engine::*;

/// Native UPlayer prefix; sufficient for UObject class checks.
#[repr(C)]
pub struct UPlayer {
    pub base: UObject,
}

unsafe impl UObjectType for UPlayer {
    const CLASS_NAME: &'static str = "Player";
}
