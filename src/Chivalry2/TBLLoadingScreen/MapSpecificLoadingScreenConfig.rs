//! Native source: TBL/Source/TBLLoadingScreen/Public/MapSpecificLoadingScreenConfig.h.

use crate::Chivalry2::Engine::FSoftObjectPath;
use crate::Chivalry2::Engine::FString;
use crate::Chivalry2::Engine::FText;
use crate::Chivalry2::Engine::TArray;

/// Raw engine-owned FText storage. Rust never destroys this view.
#[repr(C, align(8))]
pub struct FTextRaw(FText);

impl FTextRaw {
    pub(crate) fn from_str(text: &str) -> Self {
        Self(crate::Chivalry2::Engine::InvariantText::new(text).into_raw())
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

/// Rust adapter for GetDataForMap's non-DemoMode key normalization.
/// Strip through the last '/', then from the first '?'.
pub(crate) fn normalize_map_key(raw: &str) -> &str {
    let name = raw.rsplit('/').next().unwrap_or(raw);
    name.split('?').next().unwrap_or(name)
}

const _: () = assert!(size_of::<FMapSpecificLoadingScreenConfigData>() == 0x68);
const _: () = assert!(std::mem::offset_of!(FMapSpecificLoadingScreenConfigData, texture) == 0x10);
const _: () = assert!(std::mem::offset_of!(FMapSpecificLoadingScreenConfigData, text_a) == 0x28);
const _: () = assert!(std::mem::offset_of!(FMapSpecificLoadingScreenConfigData, text_b) == 0x40);
const _: () = assert!(std::mem::offset_of!(FMapSpecificLoadingScreenConfigData, extra) == 0x58);

define_pattern_resolver!(
    UMapSpecificLoadingScreenConfig__GetDataForMap,
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
