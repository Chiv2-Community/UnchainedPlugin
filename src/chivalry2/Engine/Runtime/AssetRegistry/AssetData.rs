//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/AssetRegistry/Public/AssetData.h>.

use crate::chivalry2::Engine::FName;

use std::fmt;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(non_snake_case)]
pub struct FAssetData {
    pub ObjectPath: FName,
    pub PackageName: FName,
    pub PackagePath: FName,
    pub AssetName: FName,
    pub AssetClass: FName,
    pub TagsAndValues: [u8; 0x10], // TSharedPtr<FAssetDataTagMap,0>
    pub ChunkIDs: [u8; 0x10],      // TArray<int> ?
    pub PackageFlags: u32,
    pub padding: [u8; 4], // for alignment to 0x50
}

impl fmt::Display for FAssetData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ObjectPath {}\n PackageName: {}\n PackagePath: {}\n AssetName: {}\n AssetClass: {}",
            self.ObjectPath, self.PackageName, self.PackagePath, self.AssetName, self.AssetClass
        )
    }
}
