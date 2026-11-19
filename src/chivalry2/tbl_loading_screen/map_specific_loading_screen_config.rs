//! Native source: TBL/Source/TBLLoadingScreen/Public/MapSpecificLoadingScreenConfig.h (shipping PDB).

use crate::chivalry2::engine::FSoftObjectPath;
use crate::chivalry2::engine::FString;
use crate::chivalry2::engine::FText;
use crate::chivalry2::engine::TArray;

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

define_pattern_resolver!(
    GetDataForMap,
    ["48 89 4C 24 08 55 53 57 41 56 48 8D 6C 24 C1 48 81 EC F8 00 00 00 49 8B F8 4C 8B F2"]
);

/// Prefix of `UMapSpecificLoadingScreenConfig`. Only ever used through a pointer
/// to the live engine object, never constructed or dropped in Rust.
#[repr(C)]
pub struct UMapSpecificLoadingScreenConfig {
    _uobject: [u8; 0x28],
    pub(crate) configs: TArray<FMapSpecificLoadingScreenConfigData>, // 0x28
    pub(crate) default_screen: FMapSpecificLoadingScreenConfigData,  // 0x38
}

#[unchained_macros::runtime_bindings]
impl UMapSpecificLoadingScreenConfig {
    // void UMapSpecificLoadingScreenConfig::GetDataForMap(longlong param_1,FString *param_2,void **param_3)
    //
    // NONE: the callee consumes (frees) its by-value FString arg, so the original
    // must be called exactly once.
    #[unchained_macros::bind_resolved_func(GetDataForMap)]
    pub(crate) unsafe fn get_data_for_map(
        self: *mut Self,
        map_name: *mut FString,
        data: *mut FMapSpecificLoadingScreenConfigData,
    ) {
    }
}

const _: () = assert!(std::mem::offset_of!(UMapSpecificLoadingScreenConfig, configs) == 0x28);

const _: () =
    assert!(std::mem::offset_of!(UMapSpecificLoadingScreenConfig, default_screen) == 0x38);
