//! ArgonSDK main: [UMapSpecificLoadingScreenConfig](https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBLLoadingScreen/Public/MapSpecificLoadingScreenConfig.h).

use crate::chivalry2::engine::FString;
use crate::chivalry2::engine::TArray;
use crate::chivalry2::tbl::fmap_specific_loading_screen_config_data::FMapSpecificLoadingScreenConfigData;

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
