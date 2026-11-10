//! Cache for data read from `DA_MapMarker_V2_C` map markers.
//!
//! Stub: the marker's field layout is not final yet (ticket `mod_loading-rix`).
//! Today this only finds the marker assets; reading their fields and feeding
//! consumers such as `resolvers::loading_screens` comes with that ticket.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Asset Registry class name of the V2 map marker.
pub const MAP_MARKER_V2_CLASS: &str = "DA_MapMarker_V2_C";

/// What the plugin keeps about one map marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapMarkerInfo {
    /// Object path of the marker asset.
    pub marker_path: String,
    // TODO(mod_loading-rix): fields read from the marker (map name, loading
    // screen texture, texts, ...) once the DA_MapMarker_V2 layout is provided.
}

/// Map marker data, keyed by map name.
#[derive(Debug, Default)]
pub struct MapMarkerCache {
    by_map: Mutex<HashMap<String, MapMarkerInfo>>,
    /// Marker assets found by the last scan, not yet read into `by_map`.
    marker_paths: Mutex<Vec<String>>,
}

impl MapMarkerCache {
    /// Map names are matched the way the engine's per-map lookups do it:
    /// without path or `?options`, ignoring case.
    fn key(map_name: &str) -> String {
        let name = map_name.rsplit('/').next().unwrap_or(map_name);
        name.split('?').next().unwrap_or(name).to_ascii_lowercase()
    }

    pub fn insert(&self, map_name: &str, info: MapMarkerInfo) {
        self.by_map.lock().unwrap().insert(Self::key(map_name), info);
    }

    pub fn get(&self, map_name: &str) -> Option<MapMarkerInfo> {
        self.by_map.lock().unwrap().get(&Self::key(map_name)).cloned()
    }

    pub fn clear(&self) {
        self.by_map.lock().unwrap().clear();
        self.marker_paths.lock().unwrap().clear();
    }

    /// Marker assets found by the last [`scan_map_markers`] call.
    pub fn marker_paths(&self) -> Vec<String> {
        self.marker_paths.lock().unwrap().clone()
    }
}

/// The process-wide map marker cache.
pub fn map_marker_cache() -> &'static MapMarkerCache {
    static CACHE: OnceLock<MapMarkerCache> = OnceLock::new();
    CACHE.get_or_init(MapMarkerCache::default)
}

/// Finds the `DA_MapMarker_V2_C` assets in the Asset Registry and records
/// their paths. Returns how many were found. Nothing calls this yet.
///
/// TODO(mod_loading-rix): load each marker, read its fields and `insert` them.
///
/// # Safety
/// Must run on the game thread, after mod assets were registered.
pub unsafe fn scan_map_markers() -> usize {
    let paths: Vec<String> = match crate::game::engine::get_assets_by_class(MAP_MARKER_V2_CLASS.into()) {
        Ok(assets) => assets
            .as_slice()
            .iter()
            .filter(|asset| asset.ObjectPath.comparison_index.value != 0)
            .map(|asset| asset.ObjectPath.to_string())
            .collect(),
        // A failed query means no map marker is installed.
        Err(_) => Vec::new(),
    };
    let count = paths.len();
    *map_marker_cache().marker_paths.lock().unwrap() = paths;
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_ignores_path_options_and_case() {
        let cache = MapMarkerCache::default();
        let info = MapMarkerInfo { marker_path: "/Game/Mods/Quarry/Quarry_MapMarker.Quarry_MapMarker".into() };
        cache.insert("TDM_QuarryKP", info.clone());
        assert_eq!(cache.get("/Game/Maps/tdm_quarrykp?listen"), Some(info));
        assert_eq!(cache.get("TDM_Other"), None);
        cache.clear();
        assert_eq!(cache.get("TDM_QuarryKP"), None);
    }
}
