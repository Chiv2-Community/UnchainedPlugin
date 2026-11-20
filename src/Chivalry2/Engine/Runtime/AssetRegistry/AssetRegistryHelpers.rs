//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/AssetRegistry/Public/AssetRegistryHelpers.h>.

use crate::Chivalry2::Engine::AssetRegistryScriptInterface;
use unchained_macros::bind_resolved_func;

define_pattern_resolver!(UAssetRegistryHelpers__GetAssetRegistry, [
    "48 89 5C 24 ?? 57 48 83 EC 20 48 8B F9 48 8D 15 ?? ?? ?? ?? 48 8D 4C 24 ?? 41 B8 01 00 00 00 E8 ?? ?? ?? ?? 48 8B 18 E8 ?? ?? ?? ?? 48 8B D3 48 8B C8 E8 ?? ?? ?? ?? 48 8B C8 48 8B 10 FF 52 ?? 48 85 C0"
]);

/// Static native helpers only; this is not a UObject instance layout.
pub struct UAssetRegistryHelpers {
    _private: (),
}

impl UAssetRegistryHelpers {
    /// UAssetRegistryHelpers::GetAssetRegistry, with explicit hidden return storage.
    ///
    /// # Safety
    /// Call on the engine thread after registry initialization; output must be
    /// writable native TScriptInterface storage. The returned pointers are borrowed.
    #[bind_resolved_func(self::UAssetRegistryHelpers__GetAssetRegistry)]
    pub unsafe fn GetAssetRegistry<'a>(
        output: &'a mut AssetRegistryScriptInterface,
    ) -> &'a mut AssetRegistryScriptInterface {
    }
}
