//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/AssetRegistry/Public/AssetRegistryHelpers.h>.

use crate::Chivalry2::Engine::AssetRegistryScriptInterface;

/// Static native helpers only; this is not a UObject instance layout.
#[unchained_macros_core::runtime_bindings]
pub struct UAssetRegistryHelpers {
    _private: (),
}

#[unchained_macros_kismet::kismet_runtime_bindings("/Script/AssetRegistry.AssetRegistryHelpers")]
impl UAssetRegistryHelpers {
    /// UAssetRegistryHelpers::GetAssetRegistry, with explicit hidden return storage.
    ///
    /// # Safety
    /// Call on the engine thread after registry initialization; output must be
    /// writable native TScriptInterface storage. The returned pointers are borrowed.
    pub unsafe fn GetAssetRegistry<'a>(
        output: &'a mut AssetRegistryScriptInterface,
    ) -> &'a mut AssetRegistryScriptInterface {
    }
}
