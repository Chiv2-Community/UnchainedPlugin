//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/UObject/UObjectThreadContext.h>.

use std::ffi::c_void;

#[unchained_macros_core::runtime_bindings]
pub type FUObjectSerializeContext = c_void;
