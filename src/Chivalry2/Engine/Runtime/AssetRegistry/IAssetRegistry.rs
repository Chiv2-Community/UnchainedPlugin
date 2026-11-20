//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/AssetRegistry/Public/IAssetRegistry.h>.

use crate::Chivalry2::Engine::FArchive;
use crate::Chivalry2::Engine::FName;
use crate::Chivalry2::Engine::FString;
use crate::Chivalry2::Engine::TArray;

use crate::Chivalry2::Engine::AssetRegistryScriptInterface;
use crate::Chivalry2::Engine::FAssetData;
use crate::Chivalry2::Engine::UAssetRegistryHelpers;

/// Native interface subobject, not the enclosing UObject.
// Steam/EGS table 0x144d73648; UE AssetRegistry/Public/IAssetRegistry.h.
#[rust_ctables::cpp_struct]
#[derive(Debug)]
pub struct IAssetRegistry {
    #[vtable(
        offset(0x20);
        /// # Safety
        /// Call on the game thread; output must be a writable engine-compatible array.
        GetAssetsByPath(self: *const Self, path: FName, output: *mut TArray<FAssetData>, recursive: bool, on_disk_only: bool) -> bool;
        /// # Safety
        /// Call on the game thread; output must be a writable engine-compatible array.
        GetAssetsByClass(self: *const Self, class: FName, output: *mut TArray<FAssetData>, search_subclasses: bool) -> bool;
        offset(0x148);
        /// # Safety
        /// Call on the game thread with a live array of native path strings.
        ScanPathsSynchronous(self: *mut Self, paths: *const TArray<FString>, force_rescan: bool);
        /// # Safety
        /// Call on the game thread with a live array of native filename strings.
        ScanFilesSynchronous(self: *mut Self, files: *const TArray<FString>, force_rescan: bool);
        offset(0x1f8);
        // Target UAssetRegistryImpl::Serialize reads State at +0x30 and
        // SerializationOptions at +0x2a0, then caches paths after state serialization.
        /// # Safety
        /// Call on the game thread with a live native archive and initialized registry.
        Serialize(self: *mut Self, archive: *mut FArchive);
    )]
    vtable: *const (),
}

impl IAssetRegistry {
    /// Obtain the native secondary-base pointer; an object pointer is not a fallback.
    ///
    /// # Safety
    /// Call on the game thread after native registry initialization.
    pub unsafe fn asset_registry_interface() -> Result<*mut Self, String> {
        let mut registry = AssetRegistryScriptInterface::new();
        crate::resolvers::require_function_bindings(&["UAssetRegistryHelpers__GetAssetRegistry"])?;
        UAssetRegistryHelpers::GetAssetRegistry(&mut registry);
        registry.asset_registry()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_interface_never_falls_back_to_object() {
        let mut object_marker = 0u8;
        let mut registry = AssetRegistryScriptInterface::new();
        assert!(registry.asset_registry().is_err());
        registry.object = std::ptr::addr_of_mut!(object_marker).cast();
        assert!(registry.asset_registry().is_err());
    }

    #[test]
    fn registry_uses_interface_address_independently_of_object() {
        let mut object_marker = 0u8;
        let mut interface_marker = 0u8;
        let mut registry = AssetRegistryScriptInterface::new();
        registry.interface = std::ptr::addr_of_mut!(interface_marker).cast();
        assert_eq!(registry.asset_registry().unwrap(), registry.interface);
        registry.object = std::ptr::addr_of_mut!(object_marker).cast();
        assert_eq!(registry.asset_registry().unwrap(), registry.interface);
    }
}
