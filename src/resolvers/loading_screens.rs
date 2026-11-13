//! Extra per-map loading screens.
//!
//! Hooks `UMapSpecificLoadingScreenConfig::GetDataForMap` and appends our own
//! `FMapSpecificLoadingScreenConfigData` entries to the live config's `Configs`
//! array, so the native lookup (and the native loading screen widget) serve
//! them like any other map. The entries come from `DA_MapMarker_V2` map
//! markers (see `features::map_markers`).

use crate::game::engine::{FSoftObjectPath, FText};
use crate::ue::{EFindName, FName, FString, TArray};
use std::panic::{catch_unwind, AssertUnwindSafe};

/// A per-map loading screen to inject into
/// `UMapSpecificLoadingScreenConfig::Configs`, taken from a map marker.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LoadingScreen {
    /// Map name as the native lookup sees it (path and `?options` stripped).
    map: String,
    /// Texture soft object path, without the `Texture2D'...'` wrapper.
    texture: String,
    /// Text stored at entry +0x28.
    text_a: String,
    /// Text stored at entry +0x40.
    text_b: String,
}

impl LoadingScreen {
    /// The loading screen a map marker provides for this map, if the marker
    /// (or its default) names a texture. The row's authored summary supplies the
    /// texts: its map name as the title (falling back to the map's short name)
    /// and its description as the subtitle.
    fn from_marker(info: &crate::features::map_markers::MapMarkerInfo) -> Option<Self> {
        let title = if info.title.is_empty() { info.map_name.clone() } else { info.title.clone() };
        Some(Self {
            map: info.map_name.clone(),
            texture: info.loading_screen.clone()?,
            text_a: title,
            text_b: info.subtitle.clone(),
        })
    }

    /// Finds the loading screen for a raw map name as passed to
    /// `GetDataForMap`, from the map marker cache.
    fn lookup(raw_map_name: &str) -> Option<Self> {
        let key = native_key(raw_map_name);
        crate::features::map_markers::map_marker_cache()
            .get(key)
            .as_ref()
            .and_then(Self::from_marker)
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
    unsafe fn add_entry(&mut self, extra: &LoadingScreen) -> Result<(), String> {
        if self.has_entry(&extra.map) {
            return Ok(());
        }
        let entry = FMapSpecificLoadingScreenConfigData {
            map_name: FString::from(extra.map.as_str()),
            texture: FSoftObjectPath::new(&extra.texture),
            text_a: FTextRaw::from_str(&extra.text_a)?,
            text_b: FTextRaw::from_str(&extra.text_b)?,
            extra: [0; 0x10],
        };
        self.configs.push(entry);
        sinfo!["Added extra loading screen config for {} ({} entries)", extra.map, self.configs.len()];
        Ok(())
    }
}

/// Adds the entry, turning panics and errors into log lines: neither may cross
/// into the engine.
fn add_entry_guarded(config: *mut UMapSpecificLoadingScreenConfig, extra: &LoadingScreen) {
    // SAFETY: `config` is the `this` pointer of a live `GetDataForMap` call on the game thread.
    match catch_unwind(AssertUnwindSafe(|| unsafe { (*config).add_entry(extra) })) {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            crate::swarn!("Could not add extra loading screen for {}: {}", extra.map, error)
        }
        Err(_) => crate::serror!("Panic while adding extra loading screen for {}", extra.map),
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
    extra: &LoadingScreen,
) {
    if data.texture.asset_path_name != config.default_screen.texture.asset_path_name {
        return;
    }
    data.texture.asset_path_name = FName::with_type(&extra.texture, EFindName::Add);
    sinfo!["Using fallback loading screen texture {}", extra.texture];
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
    let extra = LoadingScreen::lookup(&(*map_name).to_string());
    if let Some(extra) = &extra {
        add_entry_guarded(this_ptr, extra);
        swarn!["Extra loading screen config for {} is now present", extra.map];
    }
    CALL_ORIGINAL!(GetDataForMap(this_ptr, map_name, data));
    if let (true, Some(extra)) = (cfg!(feature = "loading_screen_texture_fallback"), &extra) {
        apply_texture_fallback(&*this_ptr, &mut *data, extra);
        swarn!["Extra loading screen texture for {} is now applied", extra.map];
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
    fn marker_loading_screen_needs_a_texture() {
        use crate::features::map_markers::MapMarkerInfo;
        let mut info = MapMarkerInfo {
            marker_path: "/Game/Mods/Quarry/Quarry_MapMarker.Quarry_MapMarker".into(),
            map_name: "TDM_Quarry".into(),
            map_path: "/Game/Mods/Quarry/TDM_Quarry.TDM_Quarry".into(),
            loading_screen: Some("/Game/Mods/Quarry/T_Load.T_Load".into()),
            title: String::new(),
            subtitle: String::new(),
            gamemode: 0,
            faction: 0,
        };
        let screen = LoadingScreen::from_marker(&info).expect("texture is set");
        assert_eq!(screen.map, "TDM_Quarry");
        assert_eq!(screen.texture, "/Game/Mods/Quarry/T_Load.T_Load");
        // No authored summary: the map's short name stands in for the title.
        assert_eq!(screen.text_a, "TDM_Quarry");
        assert_eq!(screen.text_b, "");
        info.loading_screen = None;
        assert_eq!(LoadingScreen::from_marker(&info), None);
    }

    #[test]
    fn marker_loading_screen_uses_the_authored_summary_when_present() {
        use crate::features::map_markers::MapMarkerInfo;
        let info = MapMarkerInfo {
            marker_path: "/Game/Mods/Quarry/Quarry_MapMarker.Quarry_MapMarker".into(),
            map_name: "TDM_Quarry".into(),
            map_path: "/Game/Mods/Quarry/TDM_Quarry.TDM_Quarry".into(),
            loading_screen: Some("/Game/Mods/Quarry/T_Load.T_Load".into()),
            title: "The Forgotten Quarry".into(),
            subtitle: "Dust and echoes.".into(),
            gamemode: 1,
            faction: 0,
        };
        let screen = LoadingScreen::from_marker(&info).expect("texture is set");
        assert_eq!(screen.text_a, "The Forgotten Quarry");
        assert_eq!(screen.text_b, "Dust and echoes.");
    }

    #[test]
    fn lookup_finds_nothing_for_a_map_without_marker_data() {
        assert!(LoadingScreen::lookup("/Game/Maps/Frontend/Frontend?startup").is_none());
        assert!(LoadingScreen::lookup("").is_none());
    }
}
