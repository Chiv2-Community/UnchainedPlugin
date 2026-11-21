//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/UObject/PrimaryAssetId.h>.

use std::ffi::c_void;

#[unchained_macros::runtime_bindings]
pub type FPrimaryAssetId = c_void;
