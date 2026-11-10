//! Extra per-map loading screens.
//!
//! Hooks `UMapSpecificLoadingScreenConfig::GetDataForMap` and appends our own
//! `FMapSpecificLoadingScreenConfigData` entries to the live config's `Configs`
//! array, so the native lookup (and the native loading screen widget) serve
//! them like any other map.

use crate::game::engine::{FSoftObjectPath, FText};
use crate::ue::{EFindName, FName, FString, TArray};
use std::panic::{catch_unwind, AssertUnwindSafe};

/// An extra per-map loading screen, injected into `UMapSpecificLoadingScreenConfig::Configs`.
/// TODO: load these from files instead of hard-coding.
struct ExtraLoadingScreen {
    /// Map name as the native lookup sees it (path and `?options` stripped).
    map: &'static str,
    /// Texture soft object path, without the `Texture2D'...'` wrapper.
    texture: &'static str,
    /// Text stored at entry +0x28. TODO: verify which UI label this feeds.
    text_a: &'static str,
    /// Text stored at entry +0x40. TODO: verify which UI label this feeds.
    text_b: &'static str,
}

const EXTRA_LOADING_SCREENS: &[ExtraLoadingScreen] = &[ExtraLoadingScreen {
    map: "TDM_QuarryKP",
    texture: "/Game/Mods/AgMods/KendoPlus/TO_HIPP.TO_HIPP",
    text_a: "Quarry KP",
    text_b: "",
}];

impl ExtraLoadingScreen {
    /// Finds the extra screen for a raw map name as passed to `GetDataForMap`.
    fn find(raw_map_name: &str) -> Option<&'static Self> {
        let key = native_key(raw_map_name);
        EXTRA_LOADING_SCREENS
            .iter()
            .find(|extra| extra.map.eq_ignore_ascii_case(key))
    }
}

/// Mirrors how the native `GetDataForMap` normalizes its key: drop everything
/// up to the last '/', then everything from the first '?'.
fn native_key(raw: &str) -> &str {
    let name = raw.rsplit('/').next().unwrap_or(raw);
    name.split('?').next().unwrap_or(name)
}

/// Raw engine `FText` storage (0x18 bytes: shared ref + flags). Only ever filled
/// by the engine and then moved into an engine-owned array; Rust never reads or
/// drops it.
#[repr(C, align(8))]
pub struct FTextRaw([u8; 0x18]);

impl FTextRaw {
    /// Builds an engine-owned, culture-invariant `FText` from a Rust string.
    ///
    /// # Safety
    /// Must run on the game thread after the engine allocator is available.
    unsafe fn from_str(text: &str) -> Result<Self, String> {
        use crate::resolvers::admin_control::o_FText_AsCultureInvariant;

        let mut raw = Self([0; 0x18]);
        let mut string = FString::from(text);
        let result = CALL_ORIGINAL_SAFE!(FText_AsCultureInvariant(
            std::ptr::from_mut(&mut raw).cast::<FText>(),
            &mut string
        ));
        // The callee takes its by-value FString and destroys/moves it, so ours
        // must not be dropped again.
        std::mem::forget(string);
        result.map(|_| raw)
    }
}

/// Layout of `FMapSpecificLoadingScreenConfigData`, recovered from Ghidra.
#[repr(C)]
pub struct FMapSpecificLoadingScreenConfigData {
    map_name: FString,        // 0x00
    texture: FSoftObjectPath, // 0x10
    text_a: FTextRaw,         // 0x28
    text_b: FTextRaw,         // 0x40
    /// Engine-side this is a `TArray` with 0x30-byte elements; we only leave it empty.
    extra: [u8; 0x10], // 0x58
}
const _: () = assert!(size_of::<FMapSpecificLoadingScreenConfigData>() == 0x68);
const _: () = assert!(std::mem::offset_of!(FMapSpecificLoadingScreenConfigData, texture) == 0x10);
const _: () = assert!(std::mem::offset_of!(FMapSpecificLoadingScreenConfigData, text_a) == 0x28);
const _: () = assert!(std::mem::offset_of!(FMapSpecificLoadingScreenConfigData, text_b) == 0x40);
const _: () = assert!(std::mem::offset_of!(FMapSpecificLoadingScreenConfigData, extra) == 0x58);

/// Prefix of `UMapSpecificLoadingScreenConfig`. Only ever used through a pointer
/// to the live engine object, never constructed or dropped in Rust.
#[repr(C)]
pub struct UMapSpecificLoadingScreenConfig {
    _uobject: [u8; 0x28],
    configs: TArray<FMapSpecificLoadingScreenConfigData>, // 0x28
    default_screen: FMapSpecificLoadingScreenConfigData,  // 0x38
}
const _: () = assert!(std::mem::offset_of!(UMapSpecificLoadingScreenConfig, configs) == 0x28);
const _: () = assert!(std::mem::offset_of!(UMapSpecificLoadingScreenConfig, default_screen) == 0x38);

impl UMapSpecificLoadingScreenConfig {
    fn has_entry(&self, key: &str) -> bool {
        self.configs
            .as_slice()
            .iter()
            .any(|entry| native_key(&entry.map_name.to_string()).eq_ignore_ascii_case(key))
    }

    /// Appends an entry for `extra` unless one with the same key already exists.
    ///
    /// # Safety
    /// `self` must be the live engine object; must run on the game thread.
    unsafe fn add_entry(&mut self, extra: &ExtraLoadingScreen) -> Result<(), String> {
        if self.has_entry(extra.map) {
            return Ok(());
        }
        let entry = FMapSpecificLoadingScreenConfigData {
            map_name: FString::from(extra.map),
            texture: FSoftObjectPath::new(extra.texture),
            text_a: FTextRaw::from_str(extra.text_a)?,
            text_b: FTextRaw::from_str(extra.text_b)?,
            extra: [0; 0x10],
        };
        self.configs.push(entry);
        sinfo![fn; "Added extra loading screen config for {} ({} entries)", extra.map, self.configs.len()];
        Ok(())
    }
}

/// Adds the entry, turning panics and errors into log lines: neither may cross
/// into the engine.
fn add_entry_guarded(config: *mut UMapSpecificLoadingScreenConfig, extra: &ExtraLoadingScreen) {
    // SAFETY: `config` is the `this` pointer of a live `GetDataForMap` call on the game thread.
    match catch_unwind(AssertUnwindSafe(|| unsafe { (*config).add_entry(extra) })) {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            crate::swarn!(fn; "Could not add extra loading screen for {}: {}", extra.map, error)
        }
        Err(_) => crate::serror!(fn; "Panic while adding extra loading screen for {}", extra.map),
    }
}

/// Fallback (feature `loading_screen_texture_fallback`): when the native lookup
/// fell back to the default entry, patch just the texture `FName`.
///
/// # Safety
/// `data` must be the out-struct filled by the original `GetDataForMap`.
unsafe fn apply_texture_fallback(
    config: &UMapSpecificLoadingScreenConfig,
    data: &mut FMapSpecificLoadingScreenConfigData,
    extra: &ExtraLoadingScreen,
) {
    if data.texture.asset_path_name != config.default_screen.texture.asset_path_name {
        return;
    }
    data.texture.asset_path_name = FName::with_type(extra.texture, EFindName::Add);
    sinfo![fn; "Using fallback loading screen texture {}", extra.texture];
}

define_pattern_resolver!(GetDataForMap, [
    "48 89 4C 24 08 55 53 57 41 56 48 8D 6C 24 C1 48 81 EC F8 00 00 00 49 8B F8 4C 8B F2"
]);
// void UMapSpecificLoadingScreenConfig::GetDataForMap(longlong param_1,FString *param_2,void **param_3)
//
// NONE: the callee consumes (frees) its by-value FString arg, so the original
// must be called exactly once.
CREATE_HOOK!(GetDataForMap, ACTIVE, NONE, (), (
    this_ptr: *mut UMapSpecificLoadingScreenConfig,
    map_name: *mut FString,
    data: *mut FMapSpecificLoadingScreenConfigData,
), {
    // The key must be read before the call: the callee mutates and frees `map_name`.
    let extra = ExtraLoadingScreen::find(&(*map_name).to_string());
    if let Some(extra) = extra {
        add_entry_guarded(this_ptr, extra);
    }
    CALL_ORIGINAL!(GetDataForMap(this_ptr, map_name, data));
    if let (true, Some(extra)) = (cfg!(feature = "loading_screen_texture_fallback"), extra) {
        apply_texture_fallback(&*this_ptr, &mut *data, extra);
    }
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_key_strips_path_and_options() {
        assert_eq!(native_key("TDM_QuarryKP"), "TDM_QuarryKP");
        assert_eq!(native_key("/Game/Maps/Frontend/Frontend"), "Frontend");
        assert_eq!(native_key("/Game/Mods/Foo/TDM_QuarryKP?listen?Game=TDM"), "TDM_QuarryKP");
        assert_eq!(native_key("TDM_QuarryKP?listen"), "TDM_QuarryKP");
        assert_eq!(native_key(""), "");
        assert_eq!(native_key("/Game/Maps/"), "");
    }

    #[test]
    fn find_matches_case_insensitively_on_normalized_key() {
        let expected = EXTRA_LOADING_SCREENS[0].map;
        for raw in [
            expected.to_string(),
            expected.to_ascii_lowercase(),
            format!("/Game/Mods/AgMods/KendoPlus/{expected}"),
            format!("/Game/Mods/AgMods/KendoPlus/{expected}?listen"),
        ] {
            let found = ExtraLoadingScreen::find(&raw).unwrap_or_else(|| panic!("no match for {raw}"));
            assert_eq!(found.map, expected);
        }
    }

    #[test]
    fn find_rejects_other_and_partial_maps() {
        assert!(ExtraLoadingScreen::find("Frontend").is_none());
        assert!(ExtraLoadingScreen::find("/Game/Maps/Frontend/Frontend").is_none());
        // The native lookup does substring matching; ours must stay exact.
        assert!(ExtraLoadingScreen::find("TDM_Quarry").is_none());
        assert!(ExtraLoadingScreen::find("TDM_QuarryKP_Night").is_none());
        assert!(ExtraLoadingScreen::find("").is_none());
    }

    #[test]
    fn extra_table_is_well_formed() {
        for extra in EXTRA_LOADING_SCREENS {
            assert!(!extra.map.is_empty());
            assert_eq!(native_key(extra.map), extra.map, "map must already be a bare key");
            assert!(extra.texture.starts_with('/') && extra.texture.contains('.'), "{}", extra.texture);
            assert!(!extra.texture.contains('\''), "texture must not keep the Texture2D'...' wrapper");
        }
        let mut maps: Vec<_> = EXTRA_LOADING_SCREENS.iter().map(|e| e.map.to_ascii_lowercase()).collect();
        maps.sort();
        maps.dedup();
        assert_eq!(maps.len(), EXTRA_LOADING_SCREENS.len(), "duplicate map keys");
    }
}
