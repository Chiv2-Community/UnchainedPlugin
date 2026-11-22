//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/Engine/Player.h>.

use crate::Chivalry2::Engine::*;

/// Native UPlayer prefix; sufficient for UObject class checks.
#[unchained_macros_core::runtime_bindings]
#[repr(C)]
pub struct UPlayer {
    pub base: UObject,
}

unsafe impl UObjectType for UPlayer {
    const CLASS_NAME: &'static str = "Player";
}
