//! Rust models/serializers for the mod-manager registry.
//!
//! The launcher keeps a cache at `<game>/.mod_cache/mod_manager_config.json`
//! that, for every cached mod release, embeds a `manifest` already listing the
//! pak's **maps** and **markers** with their resolved classes (e.g.
//! `object_class = "DA_ModMarker_C"`). If discovery can match a mounted pak to
//! one of these entries it can skip parsing the pak's headers entirely and
//! register the manifest's maps/markers directly.
//!
//! These models mirror the pak-manifest schema (Asset + concrete kinds, with
//! `#[serde(flatten)]` for the shared base) plus the on-disk manager-config
//! structure (`CachedMods[i].releases[j].manifest`, keyed by `hash` +
//! `pak_file_name`).

use anyhow::Context;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Schema: BaseAsset + concrete asset kinds
// ---------------------------------------------------------------------------

/// The shared `asset` base from the schema (path / hash / object_class /
/// class_path). Embedded with `#[serde(flatten)]` into the concrete kinds.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Asset {
    pub path: String,
    #[serde(default)]
    pub hash: Option<String>,
    #[serde(default)]
    pub object_class: Option<String>,
    #[serde(default)]
    pub class_path: Option<String>,
}

/// A `DA_ModMarker` / `DA_MapInfo` marker instance.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Marker {
    #[serde(flatten)]
    pub asset: Asset,
    #[serde(default)]
    pub associated_blueprints: Vec<String>,
}

/// A game map (`.umap`).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Map {
    #[serde(flatten)]
    pub asset: Asset,
    #[serde(default)]
    pub game_mode: Option<String>,
    #[serde(default)]
    pub map_name: Option<String>,
    #[serde(default)]
    pub map_description: Option<String>,
    #[serde(default)]
    pub attacking_faction: Option<String>,
    #[serde(default)]
    pub defending_faction: Option<String>,
    #[serde(default)]
    pub game_mode_type: Option<String>,
    #[serde(default)]
    pub tbl_default_gamemode: Option<String>,
}

/// A blueprint mod.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Blueprint {
    #[serde(flatten)]
    pub asset: Asset,
    #[serde(default)]
    pub mod_name: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub mod_description: Option<String>,
    #[serde(default)]
    pub mod_repo_url: Option<String>,
    #[serde(default)]
    pub silent_load: bool,
    #[serde(default)]
    pub show_in_gui: bool,
    #[serde(default)]
    pub enable_by_default: bool,
    #[serde(default)]
    pub is_client_side: bool,
    #[serde(default)]
    pub online_only: bool,
    #[serde(default)]
    pub host_only: bool,
    #[serde(default)]
    pub allow_on_frontend: bool,
    #[serde(default)]
    pub is_hidden: bool,
}

/// Any other asset.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Arbitrary {
    #[serde(flatten)]
    pub asset: Asset,
    #[serde(default)]
    pub mod_name: Option<String>,
}

/// An asset replacement (schema: plain asset).
pub type Replacement = Asset;

// ---------------------------------------------------------------------------
// Schema: Manifest
// ---------------------------------------------------------------------------

/// The per-pak asset manifest.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ModManifest {
    #[serde(default)]
    pub markers: Vec<Marker>,
    #[serde(default)]
    pub maps: Vec<Map>,
    #[serde(default)]
    pub blueprints: Vec<Blueprint>,
    #[serde(default)]
    pub replacements: Vec<Replacement>,
    #[serde(default)]
    pub arbitrary: Vec<Arbitrary>,
}

// ---------------------------------------------------------------------------
// On-disk config (mod_manager_config.json)
// ---------------------------------------------------------------------------

/// One cached mod release: the pak identity (`hash`, `pak_file_name`) plus its
/// resolved `manifest`.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ModRelease {
    #[serde(default)]
    pub tag: Option<String>,
    /// sha512 of the pak archive.
    #[serde(default)]
    pub hash: Option<String>,
    #[serde(default)]
    pub pak_file_name: Option<String>,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub info: Option<serde_json::Value>,
    #[serde(default)]
    pub manifest: ModManifest,
    #[serde(default)]
    pub release_notes_markdown: Option<String>,
    #[serde(rename = "ReleaseUrl")]
    pub release_url: Option<String>,
    #[serde(rename = "Version")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CachedMod {
    #[serde(default)]
    pub latest_release_info: Option<serde_json::Value>,
    #[serde(default)]
    pub releases: Vec<ModRelease>,
    // In the real config `LatestRelease` is an array of releases (sometimes null).
    #[serde(default, rename = "LatestRelease")]
    pub latest_release: Option<Vec<ModRelease>>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ModCoordinates {
    #[serde(rename = "Version")]
    pub version: Option<String>,
    #[serde(rename = "Org")]
    pub org: Option<String>,
    #[serde(rename = "ModuleName")]
    pub module_name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ManagedPak {
    #[serde(default)]
    pub coordinates: Option<ModCoordinates>,
    #[serde(default)]
    pub pak_file_name: Option<String>,
    #[serde(default)]
    pub priority: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct PakDir {
    #[serde(default)]
    pub dir_path: Option<String>,
    #[serde(default)]
    pub managed_paks: Vec<ManagedPak>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct EnabledModRelease {
    #[serde(rename = "Version")]
    pub version: Option<String>,
    #[serde(rename = "Org")]
    pub org: Option<String>,
    #[serde(rename = "ModuleName")]
    pub module_name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ModManagerConfig {
    #[serde(default, rename = "EnabledModReleases")]
    pub enabled_mod_releases: Vec<EnabledModRelease>,
    #[serde(default, rename = "CachedMods")]
    pub cached_mods: Vec<CachedMod>,
    #[serde(default, rename = "PakDir")]
    pub pak_dir: Option<PakDir>,
    #[serde(default, rename = "Kind")]
    pub kind: Option<String>,
}

impl ModManagerConfig {
    /// Load the config from `path`.
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let data = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read mod registry config {}", path.display()))?;
        Ok(serde_json::from_str(&data)?)
    }

    /// The default on-disk location under a game root: `<game>/.mod_cache/mod_manager_config.json`.
    pub fn default_path(game_root: &std::path::Path) -> std::path::PathBuf {
        game_root.join(".mod_cache").join("mod_manager_config.json")
    }

    /// Collect every (pak_file_name, optional sha512 hash, manifest) from all
    /// cached releases — the raw material for a pak→manifest lookup.
    pub fn release_manifests(&self) -> Vec<(&str, Option<&str>, &ModManifest)> {
        let mut out = Vec::new();
        for cached in &self.cached_mods {
            for rel in cached
                .releases
                .iter()
                .chain(cached.latest_release.as_deref().into_iter().flatten())
            {
                out.push((
                    rel.pak_file_name.as_deref().unwrap_or(""),
                    rel.hash.as_deref(),
                    &rel.manifest,
                ));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<u8> {
        let path = format!(
            "{}/test-resources/mod_registry/mod_manager_config.json",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read(path).expect("fixture should exist")
    }

    #[test]
    fn parses_manifest_config() {
        let cfg: ModManagerConfig =
            serde_json::from_slice(&fixture()).expect("config should parse");
        assert!(cfg.kind.is_some());
        assert_eq!(cfg.cached_mods.len(), 2);
    }

    #[test]
    fn reads_marker_and_map_classes() {
        let cfg: ModManagerConfig =
            serde_json::from_slice(&fixture()).expect("config should parse");
        let manifests = cfg.release_manifests();

        // BigNumbers has a DA_ModMarker_C marker.
        let bn = manifests
            .iter()
            .find(|(name, _, _)| name == &"BigNumbers.pak")
            .expect("BigNumbers release present");
        let marker = bn.2.markers.first().expect("BigNumbers has a marker");
        assert_eq!(marker.asset.object_class.as_deref(), Some("DA_ModMarker_C"));
        assert!(marker.asset.path.ends_with(".uasset"));

        // SimpleMap has a .umap map (sourced via the LatestRelease array path).
        let sm = manifests
            .iter()
            .find(|(name, _, _)| name == &"SimpleMap.pak")
            .expect("SimpleMap release present (from LatestRelease)");
        let map = sm.2.maps.first().expect("SimpleMap has a map");
        assert!(map.asset.path.ends_with(".umap"));
    }

    #[test]
    fn reads_arbitrary_class_and_latest_release_array() {
        let cfg: ModManagerConfig =
            serde_json::from_slice(&fixture()).expect("config should parse");
        let manifests = cfg.release_manifests();
        let bn = manifests
            .iter()
            .find(|(name, _, _)| name == &"BigNumbers.pak")
            .expect("BigNumbers present");
        let arb = bn.2.arbitrary.first().expect("BigNumbers has an arbitrary asset");
        assert_eq!(arb.asset.object_class.as_deref(), Some("IngameModMenuBaseWidget_C"));
        // SimpleMap was declared only under LatestRelease (array) — proves the
        // release_manifests() chaining picks it up.
        assert!(manifests.iter().any(|(n, _, _)| n == &"SimpleMap.pak"));
    }
}
