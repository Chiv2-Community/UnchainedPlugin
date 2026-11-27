//! Native source: <https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBLLoadingScreen/Public/MapSpecificLoadingScreenConfig.h>.

use super::MapSpecificLoadingScreenConfigData::FMapSpecificLoadingScreenConfigData;
use crate::Chivalry2::Engine::FString;
use crate::Chivalry2::Engine::TArray;

define_pattern_resolver!(
    UMapSpecificLoadingScreenConfig__GetDataForMap,
    ["48 89 4C 24 08 55 53 57 41 56 48 8D 6C 24 C1 48 81 EC F8 00 00 00 49 8B F8 4C 8B F2"]
);

/// Prefix of `UMapSpecificLoadingScreenConfig`. Only ever used through a pointer
/// to the live engine object, never constructed or dropped in Rust.
#[unchained_macros_core::runtime_bindings]
#[repr(C)]
pub struct UMapSpecificLoadingScreenConfig {
    _uobject: [u8; 0x28],
    pub(crate) configs: TArray<FMapSpecificLoadingScreenConfigData>, // 0x28
    pub(crate) default_screen: FMapSpecificLoadingScreenConfigData,  // 0x38
}

#[unchained_macros_core::runtime_bindings]
impl UMapSpecificLoadingScreenConfig {
    //
    // NONE: the callee consumes (frees) its by-value FString arg, so the original
    // must be called exactly once.
    #[unchained_macros_core::bind_resolved_func(UMapSpecificLoadingScreenConfig__GetDataForMap)]
    pub(crate) unsafe fn GetDataForMap(
        self: *mut Self,
        map_name: *mut FString,
        data: *mut FMapSpecificLoadingScreenConfigData,
    ) {
    }
}

const _: () = assert!(std::mem::offset_of!(UMapSpecificLoadingScreenConfig, configs) == 0x28);

const _: () =
    assert!(std::mem::offset_of!(UMapSpecificLoadingScreenConfig, default_screen) == 0x38);
