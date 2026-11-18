//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/UObject/SoftObjectPath.h>.

#![allow(non_snake_case)]
use crate::chivalry2::engine::*;

// SoftClassPath

#[repr(C)]
#[derive(Debug)]
pub struct FSoftObjectPath {
    pub asset_path_name: FName,   // 0x00
    pub sub_path_string: FString, // 0x08
}

impl FSoftObjectPath {
    pub fn new(path: &str) -> Self {
        Self {
            // A loaded object need not have its full path interned as one FName.
            asset_path_name: FName::with_type(path, EFindName::Add),
            sub_path_string: FString::default(),
        }
    }

    /// Full object path (`/Game/Pkg.Object`, plus `:SubPath` when set). Empty
    /// for an unset path. `FSoftClassPath` shares this layout.
    pub fn to_path_string(&self) -> String {
        if self.asset_path_name.comparison_index.value == 0 {
            return String::new();
        }
        let asset = self.asset_path_name.to_string();
        let sub_path = self.sub_path_string.to_string();
        if sub_path.is_empty() {
            asset
        } else {
            format!("{asset}:{sub_path}")
        }
    }
}

const _: () = assert!(std::mem::size_of::<FSoftObjectPath>() == 0x18);
