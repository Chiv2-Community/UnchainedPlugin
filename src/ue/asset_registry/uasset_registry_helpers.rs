use crate::ue::AssetRegistryScriptInterface;
use unchained_macros::bind_resolver_func;

/// Static native helpers only; this is not a UObject instance layout.
pub struct UAssetRegistryHelpers { _private: () }

impl UAssetRegistryHelpers {
    /// UAssetRegistryHelpers::GetAssetRegistry, with explicit hidden return storage.
    ///
    /// # Safety
    /// Call on the engine thread after registry initialization; output must be
    /// writable native TScriptInterface storage. The returned pointers are borrowed.
    #[bind_resolver_func(crate::resolvers::asset_registry::UAssetRegistryHelpers__GetAssetRegistry)]
    pub unsafe fn get_asset_registry<'a>(output: &'a mut AssetRegistryScriptInterface) -> &'a mut AssetRegistryScriptInterface {}
}
