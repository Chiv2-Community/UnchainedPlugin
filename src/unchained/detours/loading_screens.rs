//! Inject map-marker loading screens and optional texture fallback.

use crate::chivalry2::engine::EFindName;
use crate::chivalry2::engine::FName;
use crate::chivalry2::engine::FSoftObjectPath;
use crate::chivalry2::engine::FString;
use crate::chivalry2::tbl_loading_screen::map_specific_loading_screen_config::*;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[unchained_macros::bind_func_detour(UMapSpecificLoadingScreenConfig::get_data_for_map)]
pub(crate) unsafe fn get_data_for_map_detour(
    this: *mut UMapSpecificLoadingScreenConfig,
    map_name: *mut FString,
    data: *mut FMapSpecificLoadingScreenConfigData,
) {
    let this_ptr = this;
    // The key must be read before the call: the callee mutates and frees `map_name`.
    let extra = LoadingScreen::lookup(&(*map_name).to_string());
    if let Some(extra) = &extra {
        add_entry_guarded(this_ptr, extra);
        swarn![
            "Extra loading screen config for {} is now present",
            extra.map
        ];
    }
    unsafe {
        this.get_data_for_map(map_name, data);
    }
    if let (true, Some(extra)) = (cfg!(feature = "loading_screen_texture_fallback"), &extra) {
        apply_texture_fallback(&*this_ptr, &mut *data, extra);
        swarn![
            "Extra loading screen texture for {} is now applied",
            extra.map
        ];
    }
}

fn has_entry(config: &UMapSpecificLoadingScreenConfig, key: &str) -> bool {
    config
        .configs
        .as_slice()
        .iter()
        .any(|entry| normalize_map_key(&entry.map_name.to_string()).eq_ignore_ascii_case(key))
}

/// Appends an entry for `extra` unless one with the same key already exists.
///
/// # Safety
/// `config` must be the live engine object; must run on the game thread.
unsafe fn add_entry(config: &mut UMapSpecificLoadingScreenConfig, extra: &LoadingScreen) {
    if has_entry(config, &extra.map) {
        return;
    }
    let entry = FMapSpecificLoadingScreenConfigData::from_native_values(
        FString::from(extra.map.as_str()),
        FSoftObjectPath::new(&extra.texture),
        FTextRaw::from_str(&extra.text_a),
        FTextRaw::from_str(&extra.text_b),
    );
    config.configs.push(entry);
    sinfo![
        "Added extra loading screen config for {} ({} entries)",
        extra.map,
        config.configs.len()
    ];
}

/// A per-map loading screen to inject into
/// `UMapSpecificLoadingScreenConfig::Configs`, taken from a map marker.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LoadingScreen {
    /// Map name as the native lookup sees it (path and `?options` stripped).
    map: String,
    /// Texture soft object path, without the `Texture2D'...'` wrapper.
    texture: String,
    /// Loading-screen title.
    text_a: String,
    /// Loading-screen subtitle.
    text_b: String,
}

impl LoadingScreen {
    /// The loading screen a map marker provides for this map, if the marker
    /// (or its default) names a texture. The row's authored summary supplies the
    /// texts: its map name as the title (falling back to the map's short name)
    /// and its description as the subtitle.
    fn from_marker(info: &crate::features::map_markers::MapMarkerInfo) -> Option<Self> {
        let title = if info.title.is_empty() {
            info.map_name.clone()
        } else {
            info.title.clone()
        };
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
        let key = normalize_map_key(raw_map_name);
        crate::features::map_markers::map_marker_cache()
            .get(key)
            .as_ref()
            .and_then(Self::from_marker)
    }
}

/// Adds the entry, preventing panics from crossing into the engine.
fn add_entry_guarded(config: *mut UMapSpecificLoadingScreenConfig, extra: &LoadingScreen) {
    // SAFETY: `config` is the `this` pointer of a live `GetDataForMap` call on the game thread.
    match catch_unwind(AssertUnwindSafe(|| unsafe {
        add_entry(&mut *config, extra)
    })) {
        Ok(()) => {}
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

#[cfg(test)]
mod tests {
    use super::*;

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
