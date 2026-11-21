//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/GameFramework/SaveGame.h>.

use std::ffi::c_void;

#[unchained_macros::runtime_bindings]
pub type USaveGame = c_void;
