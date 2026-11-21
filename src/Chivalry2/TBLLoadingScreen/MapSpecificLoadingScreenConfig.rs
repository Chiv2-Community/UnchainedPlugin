//! Native source: <https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBLLoadingScreen/Public/MapSpecificLoadingScreenConfig.h>.
//! Native owner: TBL/Source/TBLLoadingScreen/Public/MapSpecificLoadingScreenConfig.h.

use super::MapSpecificLoadingScreenConfigData::FMapSpecificLoadingScreenConfigData;
use crate::Chivalry2::Engine::FString;
use crate::Chivalry2::Engine::TArray;

/// Rust adapter for GetDataForMap's non-DemoMode key normalization.
/// Strip through the last '/', then from the first '?'.
pub(crate) fn normalize_map_key(raw: &str) -> &str {
    let name = raw.rsplit('/').next().unwrap_or(raw);
    name.split('?').next().unwrap_or(name)
}

define_pattern_resolver!(
    UMapSpecificLoadingScreenConfig__GetDataForMap,
    ["48 89 4C 24 08 55 53 57 41 56 48 8D 6C 24 C1 48 81 EC F8 00 00 00 49 8B F8 4C 8B F2"]
);

/// Prefix of `UMapSpecificLoadingScreenConfig`. Only ever used through a pointer
/// to the live engine object, never constructed or dropped in Rust.
#[unchained_macros::runtime_bindings]
#[repr(C)]
pub struct UMapSpecificLoadingScreenConfig {
    _uobject: [u8; 0x28],
    pub(crate) configs: TArray<FMapSpecificLoadingScreenConfigData>, // 0x28
    pub(crate) default_screen: FMapSpecificLoadingScreenConfigData,  // 0x38
}

#[unchained_macros::runtime_bindings]
impl UMapSpecificLoadingScreenConfig {
    //
    // NONE: the callee consumes (frees) its by-value FString arg, so the original
    // must be called exactly once.
    #[unchained_macros::bind_resolved_func(UMapSpecificLoadingScreenConfig__GetDataForMap)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_key_strips_path_then_options() {
        for (raw, expected) in [
            ("TDM_QuarryKP", "TDM_QuarryKP"),
            ("/Game/Maps/Frontend/Frontend", "Frontend"),
            ("/Game/Mods/Foo/TDM_QuarryKP?listen?Game=TDM", "TDM_QuarryKP"),
            ("TDM_QuarryKP?listen", "TDM_QuarryKP"),
            ("", ""),
            ("/Game/Maps/", ""),
            ("?listen", ""),
            ("/Game?option/Map?listen", "Map"),
            ("/Game/Maps/地图?listen", "地图"),
        ] {
            let key = normalize_map_key(raw);
            assert_eq!(key, expected);
            // Normalization borrows the input rather than allocating.
            let start = raw.as_ptr() as usize;
            let key_start = key.as_ptr() as usize;
            assert!(key_start >= start && key_start + key.len() <= start + raw.len());
        }
    }

}
