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

// ---------------------------------------------------------------------------
// Fast-path lookup: mounted pak filename -> manifest
// ---------------------------------------------------------------------------

use std::collections::HashMap;

/// Strip the launcher's load-order prefix used on managed pak filenames, e.g.
/// `qv__-__BigNumbers.pak` → `BigNumbers.pak` (split on `__-__`, keep last part).
fn strip_load_prefix(name: &str) -> &str {
    name.rsplit("__-__").next().unwrap_or(name)
}

/// Normalize a pak filename to a comparable key: drop `.pak`, the `__-__`
/// prefix, and a trailing ` (N)` duplicate suffix; lowercase.
fn normalize_pak_name(name: &str) -> String {
    let mut base = name.strip_suffix(".pak").unwrap_or(name).trim();
    base = strip_load_prefix(base).trim();
    if let Some(idx) = base.rfind(" (") {
        if base[idx + 2..].ends_with(')') {
            base = &base[..idx];
        }
    }
    base.to_lowercase()
}

/// Pak-name → manifest lookup over the manager's cached mod releases, so a
/// mounted pak that is in the registry can skip header parsing.
#[derive(Default)]
pub struct ModRegistry {
    by_name: HashMap<String, ModManifest>,
    by_hash: HashMap<String, ModManifest>,
}

impl ModRegistry {
    /// Load the registry from a `mod_manager_config.json` path.
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let cfg = ModManagerConfig::load(path)?;
        let mut by_name = HashMap::new();
        let mut by_hash = HashMap::new();
        for (name, hash, manifest) in cfg.release_manifests() {
            let key = normalize_pak_name(name);
            if !key.is_empty() {
                by_name.entry(key).or_insert_with(|| manifest.clone());
            }
            if let Some(h) = hash.filter(|h| !h.is_empty()) {
                by_hash.entry(h.to_lowercase()).or_insert_with(|| manifest.clone());
            }
        }
        Ok(Self { by_name, by_hash })
    }

    /// Number of distinct pak manifests known.
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    /// Find the manifest for a mounted pak filename (mangle-aware).
    pub fn manifest_for_pak(&self, file_name: &str) -> Option<&ModManifest> {
        let key = normalize_pak_name(file_name);
        if let Some(m) = self.by_name.get(&key) {
            return Some(m);
        }
        // Rare case: a shorter/cleaner config name is a suffix of a mangled one.
        for (k, m) in &self.by_name {
            if k.len() < key.len() && key.ends_with(k) {
                return Some(m);
            }
        }
        None
    }

    /// Find the manifest for a pak by its SHA-512 hex digest (the launcher's key).
    pub fn manifest_for_hash(&self, sha512_hex: &str) -> Option<&ModManifest> {
        self.by_hash.get(&sha512_hex.to_lowercase())
    }
}

/// SHA-512 of a file, read in 1 MiB chunks (larger reads cut syscall overhead at
/// multi-GB sizes; digest is unchanged). Matches the launcher's `get_file_sha512`.
pub fn compute_sha512(path: &std::path::Path) -> anyhow::Result<String> {
    use sha2::{Digest, Sha512};
    use std::io::Read;
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("failed to open {} for hashing", path.display()))?;
    let mut hasher = Sha512::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// SHA-512 of each path, computed in parallel across worker threads (strided,
/// deterministic order). Each element is `Some(hex)` on success.
pub fn compute_sha512_parallel(
    paths: &[&std::path::Path],
    threads: usize,
) -> Vec<Option<String>> {
    let n = paths.len();
    let threads = threads.min(n).max(1);
    let mut results: Vec<(usize, Option<String>)> = std::thread::scope(|s| {
        let mut all: Vec<(usize, Option<String>)> = Vec::new();
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                let paths = &paths;
                s.spawn(move || {
                    let mut out: Vec<(usize, Option<String>)> = Vec::new();
                    let mut i = t;
                    while i < paths.len() {
                        out.push((i, compute_sha512(paths[i]).ok()));
                        i += threads;
                    }
                    out
                })
            })
            .collect();
        for h in handles {
            all.extend(h.join().expect("sha worker panicked"));
        }
        all
    });
    results.sort_by_key(|(i, _)| *i);
    results.into_iter().map(|(_, v)| v).collect()
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

    #[test]
    fn matches_mangled_pak_names() {
        let path = format!(
            "{}/test-resources/mod_registry/mod_manager_config.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let reg = ModRegistry::load(std::path::Path::new(&path)).expect("registry loads");
        assert_eq!(reg.len(), 2);
        assert!(reg.manifest_for_pak("qv__-__BigNumbers.pak").is_some());
        assert!(reg.manifest_for_pak("qw__-__BigNumbers (1).pak").is_some());
        assert!(reg.manifest_for_pak("BigNumbers.pak").is_some());
        assert!(reg.manifest_for_pak("SimpleMap.pak").is_some());
        assert!(reg.manifest_for_pak("NihiMaps.pak").is_none(), "unknown pak not matched");
    }

    #[test]
    fn parallel_sha_matches_sequential() {
        let path = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/test-resources/mod_registry/mod_manager_config.json"
        ));
        let seq = compute_sha512(path).expect("seq hash");
        let par = compute_sha512_parallel(&[path], 2)[0].clone().expect("par hash");
        assert_eq!(seq, par, "parallel hashing must produce the same digest");
        // 64-byte (128 hex char) sha512 digest.
        assert_eq!(seq.len(), 128);
    }
}
