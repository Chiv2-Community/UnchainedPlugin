//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/AssetRegistry/Private/AssetRegistry.h>.

use std::ffi::c_void;

#[unchained_macros_core::runtime_bindings]
pub type UAssetRegistryImpl = c_void;
