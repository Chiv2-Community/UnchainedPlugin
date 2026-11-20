//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/UObject/CoreNet.h>.

use std::ffi::c_void;

#[unchained_macros::runtime_bindings]
pub type UPackageMap = c_void;
