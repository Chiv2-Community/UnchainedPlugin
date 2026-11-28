//! Native source: <https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBLLoadingScreen/Public/MapSpecificLoadingScreenConfigData.h>.

use crate::Chivalry2::Engine::FSoftObjectPath;
use crate::Chivalry2::Engine::FString;
use crate::Chivalry2::Engine::FText;

/// Raw engine-owned FText storage. Rust never destroys this view.
#[repr(C, align(8))]
pub struct FTextRaw(FText);

impl FTextRaw {
    pub(crate) fn from_str(text: &str) -> Self {
        Self(crate::Chivalry2::Engine::InvariantText::new(text).into_raw())
    }
}

/// Layout recovered from Ghidra; filled or moved into engine-owned arrays.
#[unchained_macros_core::runtime_bindings]
#[repr(C)]
pub struct FMapSpecificLoadingScreenConfigData {
    pub(crate) map_name: FString,
    pub(crate) texture: FSoftObjectPath,
    pub(crate) text_a: FTextRaw,
    pub(crate) text_b: FTextRaw,
    /// Native TArray with 0x30-byte elements; Rust leaves it empty.
    extra: [u8; 0x10],
}

impl FMapSpecificLoadingScreenConfigData {
    /// Rust construction adapter, not a verified native constructor.
    /// Moves already-native values and preserves the existing zero-tail behavior.
    pub(crate) fn from_native_values(
        map_name: FString,
        texture: FSoftObjectPath,
        text_a: FTextRaw,
        text_b: FTextRaw,
    ) -> Self {
        Self {
            map_name,
            texture,
            text_a,
            text_b,
            extra: [0; 0x10],
        }
    }
}

const _: () = assert!(size_of::<FMapSpecificLoadingScreenConfigData>() == 0x68);
const _: () = assert!(std::mem::offset_of!(FMapSpecificLoadingScreenConfigData, texture) == 0x10);
const _: () = assert!(std::mem::offset_of!(FMapSpecificLoadingScreenConfigData, text_a) == 0x28);
const _: () = assert!(std::mem::offset_of!(FMapSpecificLoadingScreenConfigData, text_b) == 0x40);
const _: () = assert!(std::mem::offset_of!(FMapSpecificLoadingScreenConfigData, extra) == 0x58);
