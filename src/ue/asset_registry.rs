use super::{FArchive, FName, FString, TArray};
use crate::resolvers::asset_registry::FAssetData;

/// Native interface subobject, not the enclosing UObject.
// Steam/EGS table 0x144d73648; UE AssetRegistry/Public/IAssetRegistry.h.
#[rust_ctables::cpp_struct]
#[derive(Debug)]
pub struct IAssetRegistry {
    #[vtable(
        offset(0x20);
        /// # Safety
        /// Call on the game thread; output must be a writable engine-compatible array.
        get_assets_by_path(this: *const Self, path: FName, output: *mut TArray<FAssetData>, recursive: bool, on_disk_only: bool) -> bool;
        /// # Safety
        /// Call on the game thread; output must be a writable engine-compatible array.
        get_assets_by_class(this: *const Self, class: FName, output: *mut TArray<FAssetData>, search_subclasses: bool) -> bool;
        offset(0x148);
        /// # Safety
        /// Call on the game thread with a live array of native path strings.
        scan_paths_synchronous(this: *mut Self, paths: *const TArray<FString>, force_rescan: bool);
        /// # Safety
        /// Call on the game thread with a live array of native filename strings.
        scan_files_synchronous(this: *mut Self, files: *const TArray<FString>, force_rescan: bool);
        offset(0x1f8);
        /// # Safety
        /// Call on the game thread with a live native archive and initialized registry.
        serialize(this: *mut Self, archive: *mut FArchive);
    )]
    vtable: *const (),
}
