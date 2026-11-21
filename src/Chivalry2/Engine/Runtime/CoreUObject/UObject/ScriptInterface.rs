//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/UObject/ScriptInterface.h>.

use crate::Chivalry2::Engine::UObject;

use crate::Chivalry2::Engine::IAssetRegistry;

/// Native TScriptInterface<IAssetRegistry> return storage; owns neither pointer.
#[unchained_macros::runtime_bindings]
#[repr(C)]
#[derive(Debug)]
pub struct AssetRegistryScriptInterface {
    pub object: *mut UObject,
    pub interface: *mut IAssetRegistry,
}

impl AssetRegistryScriptInterface {
    /// Return the borrowed interface, never the enclosing UObject address.
    pub fn asset_registry(&self) -> Result<*mut IAssetRegistry, String> {
        if self.interface.is_null() {
            return Err("Asset Registry singleton has no native interface pointer".into());
        }
        Ok(self.interface)
    }

    pub fn new() -> Self {
        Self {
            object: std::ptr::null_mut(),
            interface: std::ptr::null_mut(),
        }
    }
}
