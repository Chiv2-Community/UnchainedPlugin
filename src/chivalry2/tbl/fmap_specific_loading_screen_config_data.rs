//! ArgonSDK main: [FMapSpecificLoadingScreenConfigData](https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBLLoadingScreen/Public/MapSpecificLoadingScreenConfigData.h).

use crate::chivalry2::engine::{FSoftObjectPath, FString, FText};

/// Raw engine-owned FText storage. Rust never destroys this view.
#[repr(C, align(8))]
pub struct FTextRaw(FText);

impl FTextRaw {
    pub(crate) fn from_str(text: &str) -> Self {
        Self(crate::chivalry2::engine::InvariantText::new(text).into_raw())
    }
}

/// Layout recovered from Ghidra; filled or moved into engine-owned arrays.
#[repr(C)]
pub struct FMapSpecificLoadingScreenConfigData {
    pub(crate) map_name: FString,
    pub(crate) texture: FSoftObjectPath,
    pub(crate) text_a: FTextRaw,
    pub(crate) text_b: FTextRaw,
    /// Native TArray with 0x30-byte elements; Rust leaves it empty.
    pub(crate) extra: [u8; 0x10],
}

const _: () = assert!(size_of::<FMapSpecificLoadingScreenConfigData>() == 0x68);
const _: () = assert!(std::mem::offset_of!(FMapSpecificLoadingScreenConfigData, texture) == 0x10);
const _: () = assert!(std::mem::offset_of!(FMapSpecificLoadingScreenConfigData, text_a) == 0x28);
const _: () = assert!(std::mem::offset_of!(FMapSpecificLoadingScreenConfigData, text_b) == 0x40);
const _: () = assert!(std::mem::offset_of!(FMapSpecificLoadingScreenConfigData, extra) == 0x58);
