//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/Engine/AssetManager.h>.

use crate::chivalry2::engine::IAssetRegistry;

use unchained_macros::bind_resolved_func;

define_pattern_resolver!(
    GetAssetRegistry,
    ["40 57 48 83 EC 20 48 8B 81 ?? ?? ?? ?? 48 8B F9 48 85 C0 75 ?? 44 8D 40 ??"]
);

/// Opaque native asset manager. No Rust allocation or layout is assumed.
#[repr(C)]
pub struct UAssetManager {
    _opaque: [u8; 0],
}

impl UAssetManager {
    /// # Safety
    /// Self must be a live native asset manager; call on the engine thread.
    #[bind_resolved_func(self::GetAssetRegistry)]
    pub unsafe fn get_asset_registry(self: *const Self) -> *mut IAssetRegistry {}
}
