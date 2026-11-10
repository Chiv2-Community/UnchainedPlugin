//! Cache of data read from `DA_MapMarker_V2_C` map markers.
//!
//! A map marker lists the maps a mod ships (`ShippedMapPaths`), each with an
//! optional loading screen texture, plus a `DefaultLoadingScreen` for maps
//! without their own. [`scan_map_markers`] reads every marker in the Asset
//! Registry into [`map_marker_cache`]; `resolvers::loading_screens` looks maps
//! up there.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::game::unchained::MapMarkerOffsets;

/// Asset Registry class name of the V2 map marker.
pub const MAP_MARKER_V2_CLASS: &str = "DA_MapMarker_V2_C";

/// What the plugin keeps about one map shipped by a map marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapMarkerInfo {
    /// Object path of the marker asset the map came from.
    pub marker_path: String,
    /// Map name without path, e.g. `TDM_QuarryKP`.
    pub map_name: String,
    /// Object path of the map, e.g. `/Game/Mods/Quarry/TDM_QuarryKP.TDM_QuarryKP`.
    pub map_path: String,
    /// Loading screen texture object path: the map's own, else the marker's
    /// default. `None` if the marker sets neither.
    pub loading_screen: Option<String>,
}

/// Map name of a map object or package path: the last path segment without
/// the `.Object` suffix or `?options`.
pub fn map_name_from_path(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    let name = name.split('?').next().unwrap_or(name);
    name.split('.').next().unwrap_or(name)
}

/// One marker's fields as read from the engine object.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapMarkerData {
    pub marker_path: String,
    /// Empty if unset.
    pub default_loading_screen: String,
    /// `(map object path, loading screen texture path or empty)`.
    pub shipped_maps: Vec<(String, String)>,
}

impl MapMarkerData {
    /// The cache entries this marker contributes, one per shipped map.
    pub fn map_infos(&self) -> Vec<MapMarkerInfo> {
        self.shipped_maps
            .iter()
            .filter(|(map_path, _)| !map_name_from_path(map_path).is_empty())
            .map(|(map_path, texture)| {
                let texture = if texture.is_empty() { &self.default_loading_screen } else { texture };
                MapMarkerInfo {
                    marker_path: self.marker_path.clone(),
                    map_name: map_name_from_path(map_path).to_string(),
                    map_path: map_path.clone(),
                    loading_screen: (!texture.is_empty()).then(|| texture.clone()),
                }
            })
            .collect()
    }
}

/// Map marker data, keyed by map name (case-insensitive).
#[derive(Debug, Default)]
pub struct MapMarkerCache {
    by_map: Mutex<HashMap<String, MapMarkerInfo>>,
}

impl MapMarkerCache {
    fn key(map_name: &str) -> String {
        map_name_from_path(map_name).to_ascii_lowercase()
    }

    /// Adds an entry unless the map already has one. Returns whether it was added.
    pub fn insert(&self, info: MapMarkerInfo) -> bool {
        let mut by_map = self.by_map.lock().unwrap();
        let key = Self::key(&info.map_name);
        if by_map.contains_key(&key) {
            return false;
        }
        by_map.insert(key, info);
        true
    }

    /// Looks a map up by name or path (`?options` and `.Object` are ignored).
    pub fn get(&self, map_name: &str) -> Option<MapMarkerInfo> {
        self.by_map.lock().unwrap().get(&Self::key(map_name)).cloned()
    }

    pub fn len(&self) -> usize {
        self.by_map.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&self) {
        self.by_map.lock().unwrap().clear();
    }
}

/// The process-wide map marker cache.
pub fn map_marker_cache() -> &'static MapMarkerCache {
    static CACHE: OnceLock<MapMarkerCache> = OnceLock::new();
    CACHE.get_or_init(MapMarkerCache::default)
}

/// Rebuilds the cache from every `DA_MapMarker_V2_C` asset in the Asset
/// Registry. Loads the marker assets (small, soft references only). Returns the
/// number of maps cached.
///
/// # Safety
/// Must run on the game thread, after mod assets were registered.
pub unsafe fn scan_map_markers() -> usize {
    // A failed query means no map marker is installed.
    let marker_paths: Vec<String> = match crate::game::engine::get_assets_by_class(MAP_MARKER_V2_CLASS.into()) {
        Ok(assets) => assets
            .as_slice()
            .iter()
            .filter(|asset| asset.ObjectPath.comparison_index.value != 0)
            .map(|asset| asset.ObjectPath.to_string())
            .collect(),
        Err(_) => Vec::new(),
    };

    let cache = map_marker_cache();
    cache.clear();
    // Offsets are per class; every map marker normally shares one class.
    let mut offsets_by_class: HashMap<usize, Option<MapMarkerOffsets>> = HashMap::new();
    for marker_path in &marker_paths {
        let marker = crate::game::engine::get_uobject_from_path(marker_path, true);
        if marker.is_null() {
            crate::swarn!("Could not load map marker {}", marker_path);
            continue;
        }
        let marker_class = (*marker).uobject_base_utility.uobject_base.class_private;
        let offsets = *offsets_by_class.entry(marker_class as usize).or_insert_with(|| {
            match MapMarkerOffsets::resolve(marker_class) {
                Ok(offsets) => Some(offsets),
                Err(error) => {
                    crate::serror!("Cannot read map markers of this class: {}", error);
                    None
                }
            }
        });
        let Some(offsets) = offsets else { continue };

        let data = MapMarkerData {
            marker_path: marker_path.clone(),
            default_loading_screen: offsets.default_loading_screen(marker),
            shipped_maps: offsets.shipped_map_paths(marker),
        };
        crate::sdebug!(
            "Map marker {}: {} shipped map(s), default loading screen {:?}",
            marker_path,
            data.shipped_maps.len(),
            data.default_loading_screen
        );
        for info in data.map_infos() {
            let (map_name, map_path) = (info.map_name.clone(), info.map_path.clone());
            if !cache.insert(info) {
                crate::swarn!(
                    "Map marker {} lists {} ({}), which another marker already provides; keeping the first",
                    marker_path, map_name, map_path
                );
            }
        }
    }
    crate::sdebug!("[mapmarkers] cached {} map(s) from {} map marker(s)", cache.len(), marker_paths.len());
    cache.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MARKER: &str = "/Game/Mods/Quarry/Quarry_MapMarker.Quarry_MapMarker";
    const MAP: &str = "/Game/Mods/Quarry/TDM_QuarryKP.TDM_QuarryKP";

    #[test]
    fn map_name_drops_path_object_suffix_and_options() {
        assert_eq!(map_name_from_path(MAP), "TDM_QuarryKP");
        assert_eq!(map_name_from_path("/Game/Mods/Quarry/TDM_QuarryKP"), "TDM_QuarryKP");
        assert_eq!(map_name_from_path("TDM_QuarryKP?listen?Game=TDM"), "TDM_QuarryKP");
        assert_eq!(map_name_from_path("/Game/Mods/Quarry/"), "");
    }

    #[test]
    fn map_without_own_texture_uses_the_marker_default() {
        let data = MapMarkerData {
            marker_path: MARKER.into(),
            default_loading_screen: "/Game/Mods/Quarry/T_Default.T_Default".into(),
            shipped_maps: vec![
                (MAP.into(), String::new()),
                ("/Game/Mods/Quarry/FFA_Quarry.FFA_Quarry".into(), "/Game/Mods/Quarry/T_FFA.T_FFA".into()),
            ],
        };
        let infos = data.map_infos();
        assert_eq!(infos.len(), 2);
        assert_eq!(infos[0].map_name, "TDM_QuarryKP");
        assert_eq!(infos[0].loading_screen.as_deref(), Some("/Game/Mods/Quarry/T_Default.T_Default"));
        assert_eq!(infos[1].loading_screen.as_deref(), Some("/Game/Mods/Quarry/T_FFA.T_FFA"));
    }

    #[test]
    fn map_has_no_loading_screen_when_neither_is_set() {
        let data = MapMarkerData {
            marker_path: MARKER.into(),
            default_loading_screen: String::new(),
            shipped_maps: vec![(MAP.into(), String::new())],
        };
        assert_eq!(data.map_infos()[0].loading_screen, None);
    }

    #[test]
    fn cache_lookup_ignores_path_options_and_case_and_keeps_the_first_entry() {
        let cache = MapMarkerCache::default();
        let info = |texture: &str| MapMarkerInfo {
            marker_path: MARKER.into(),
            map_name: "TDM_QuarryKP".into(),
            map_path: MAP.into(),
            loading_screen: Some(texture.into()),
        };
        assert!(cache.insert(info("/Game/A.A")));
        assert!(!cache.insert(info("/Game/B.B")));
        assert_eq!(cache.get("/Game/Mods/Quarry/tdm_quarrykp?listen"), Some(info("/Game/A.A")));
        assert_eq!(cache.get("TDM_Other"), None);
        cache.clear();
        assert!(cache.is_empty());
    }
}
