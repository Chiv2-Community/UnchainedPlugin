//! Off-thread mod-map discovery from the loader's mounted mod `.pak` files.
//!
//! Pure Rust: pak indexes are read with `repak` and `.umap` entries under
//! [`MOD_ROOT`] are turned into in-engine package paths plus the
//! physical-relative pak paths (`mount_point + entry`) that UE's
//! `FPackageName::FilenameToLongPackageName` maps back to the same package
//! names during registration (the runtime reader derives `PackageName` from
//! the filename passed to `ScanFilesSynchronous`).
//!
//! This module performs ZERO engine calls and is safe to run on a worker
//! thread. Registration into the live Asset Registry happens separately on
//! the game thread (see `game::engine::get_assets_by_path` and the
//! `ScanPathsAndFilesSynchronous` resolver work).
//!
//! NOTE: the `repak` crate is pulled from https://github.com/trumank/repak —
//! the crates.io crate of that name is an unrelated name-squatter.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// The asset path all discovered mod maps must live under.
pub const MOD_ROOT: &str = "/Game/Mods";

/// Segment that marks a physical content root; everything after the first
/// `.../Content/` maps to `/Game/...` (standard UE mount-point convention).
const CONTENT_SEGMENT: &str = "/Content/";

/// Characters rejected in long package names; mirrors the FPackageName rules
/// already mirrored in `features::mod_management` (`UE_INVALID_LONG_PACKAGE_CHARS`).
const INVALID_PACKAGE_CHARS: &str = "\\:*?\"<>|' ,.&!~\n\r\t@#;";

/// Source of mod pak files to scan.
#[derive(Debug, Clone)]
pub enum MapSource {
    /// A directory whose `*.pak` files are scanned (the loader's mod dir).
    PakDir(PathBuf),
    /// Explicit pak files.
    PakFiles(Vec<PathBuf>),
}

/// One discovered mod map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredModMap {
    /// In-engine package path, e.g. `/Game/Mods/ModName/Maps/MyMap`.
    pub package_path: String,
    /// Physical-relative pak path, e.g.
    /// `../../../TBL/Content/Mods/ModName/Maps/MyMap.umap`. This is the string
    /// to feed the registration scan; the engine derives `PackageName` from it.
    pub file_path: String,
}

/// Result of a discovery pass.
#[derive(Debug, Clone, Default)]
pub struct ModMapScan {
    pub maps: Vec<DiscoveredModMap>,
    /// Paks skipped during the pass with the reason (unparseable, encrypted
    /// index without a key, ...).
    pub skipped: Vec<(String, String)>,
}

pub type OnScanDone = Box<dyn FnOnce(Result<ModMapScan>) + Send>;

/// Runs a discovery pass on a worker thread (tokio `spawn_blocking`) and
/// delivers the result to `on_done`. Never touches the engine.
pub fn discover_mod_maps_async(source: MapSource, aes_key: Option<String>, on_done: OnScanDone) {
    crate::features::tokio_runtime::TOKIO_RUNTIME.spawn(async move {
        let joined = tokio::task::spawn_blocking(move || scan_mod_maps(source, aes_key)).await;
        on_done(joined
            .context("mod map scan task panicked or was cancelled")
            .and_then(|r| r));
    });
}

/// Synchronous discovery pass. No engine calls; safe on any thread.
pub fn scan_mod_maps(source: MapSource, aes_key: Option<String>) -> Result<ModMapScan> {
    let pak_paths = match source {
        MapSource::PakDir(dir) => list_pak_files(&dir)?,
        MapSource::PakFiles(files) => files,
    };

    let key = aes_key.map(|k| decode_aes_key(&k)).transpose()?;

    let mut scan = ModMapScan::default();
    let mut seen_packages: HashSet<String> = HashSet::new();

    for pak_path in pak_paths {
        let name = pak_path.display().to_string();
        match read_pak_mod_maps(&pak_path, key.as_ref()) {
            Ok(maps) => {
                let mut found = 0usize;
                for map in maps {
                    // The same package may exist in several mounted paks; the
                    // first mount wins and the rest are ignored.
                    if seen_packages.insert(map.package_path.clone()) {
                        scan.maps.push(map);
                        found += 1;
                    }
                }
                if found == 0 {
                    sinfo!(fn; "No new .umap entries under {} in {}", MOD_ROOT, name);
                } else {
                    sinfo!(fn; "Found {} mod map(s) in {}", found, name);
                }
            }
            Err(e) => {
                swarn!(fn; "Skipping pak {}: {}", name, e);
                scan.skipped.push((name, e.to_string()));
            }
        }
    }

    Ok(scan)
}

/// Reads one pak index and collects its `.umap` entries under [`MOD_ROOT`].
fn read_pak_mod_maps(pak_path: &Path, key: Option<&aes::Aes256>) -> Result<Vec<DiscoveredModMap>> {
    let mut file = std::fs::File::open(pak_path)
        .with_context(|| format!("failed to open {}", pak_path.display()))?;

    let builder = repak::PakBuilder::new();
    let builder = match key {
        Some(k) => builder.key(k.clone()),
        None => builder,
    };
    let reader = builder.reader(&mut file).context(
        "failed to parse pak index (unsupported version, or encrypted index without a key)",
    )?;

    let mount_point = reader.mount_point();
    let mut maps = Vec::new();
    for entry in reader.files() {
        if strip_umap(&entry).is_none() {
            continue;
        }
        // Index entries are relative to the mount point; the engine-visible
        // path is mount_point + entry.
        let full_path = format!("{}{}", mount_point, entry);
        let Some(package_path) = physical_path_to_package_path(&full_path) else {
            swarn!(
                fn;
                "Skipping .umap entry without a {} root: {}{}",
                CONTENT_SEGMENT,
                mount_point,
                entry
            );
            continue;
        };
        if !package_path.starts_with(MOD_ROOT) {
            continue; // Out of scope (game maps etc.); not an error.
        }
        maps.push(DiscoveredModMap {
            package_path,
            file_path: full_path,
        });
    }
    Ok(maps)
}

/// Lists `*.pak` files in `dir`, sorted, with an explicit guard against the
/// game's own pak naming scheme.
fn list_pak_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut paks = Vec::new();
    let entries = std::fs::read_dir(dir)
        .with_context(|| format!("failed to read mod dir {}", dir.display()))?;
    for entry in entries {
        let entry = entry.with_context(|| format!("failed to iterate {}", dir.display()))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let is_pak = path
            .extension()
            .map(|e| e.eq_ignore_ascii_case("pak"))
            .unwrap_or(false);
        if !is_pak {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().to_lowercase();
        if file_name.starts_with("pakchunk") {
            swarn!(fn; "Skipping game pak {}", path.display());
            continue;
        }
        paks.push(path);
    }
    paks.sort();
    Ok(paks)
}

/// Maps a physical pak path to the in-engine package path: the first
/// `.../Content/` segment maps to `/Game/`, the `.umap` extension is stripped.
/// Returns `None` for paths without a content root or with invalid package
/// characters (callers log these instead of registering garbage).
fn physical_path_to_package_path(full_path: &str) -> Option<String> {
    let normalized = full_path.replace('\\', "/");
    let content_start = normalized.find(CONTENT_SEGMENT)? + CONTENT_SEGMENT.len();
    let remainder = normalized.get(content_start..)?;
    let stem = strip_umap(remainder)?;
    let package_path = format!("/Game/{}", stem);
    if package_path.starts_with(MOD_ROOT)
        && !package_path.contains("//")
        && !package_path.contains('\0')
        && !package_path.ends_with('/')
        && !package_path.chars().any(|c| INVALID_PACKAGE_CHARS.contains(c))
    {
        Some(package_path)
    } else {
        None
    }
}

/// Case-insensitive `.umap` suffix check + strip.
fn strip_umap(name: &str) -> Option<&str> {
    match name.get(name.len().saturating_sub(5)..) {
        Some(tail) if tail.eq_ignore_ascii_case(".umap") => Some(&name[..name.len() - 5]),
        _ => None,
    }
}

/// Decodes an AES-256 key given as base64 or hex (optionally `0x`-prefixed),
/// matching the repak CLI convention.
fn decode_aes_key(key: &str) -> Result<aes::Aes256> {
    let trimmed = key.trim();
    let bytes = if let Some(hex_part) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        hex::decode(hex_part).context("invalid hex AES key")?
    } else {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD
            .decode(trimmed)
            .or_else(|_| hex::decode(trimmed))
            .context("AES key must be base64 or hex")?
    };
    aes::Aes256::new_from_slice(&bytes).context("AES key must decode to 32 bytes")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Packs a small v9 pak with repak's writer and returns its path.
    fn write_test_pak(dir: &Path, mount_point: &str, entries: &[(&str, &[u8])]) -> PathBuf {
        let pak_path = dir.join("test_mod.pak");
        let file = std::fs::File::create(&pak_path).expect("create test pak");
        let mut writer = repak::PakBuilder::new().writer(
            file,
            repak::Version::V9,
            mount_point.to_string(),
            None,
        );
        for (path, data) in entries {
            writer.write_file(path, false, *data).expect("write entry");
        }
        writer.write_index().expect("write index");
        pak_path
    }

    #[test]
    fn discovers_umap_entries_under_mod_root() {
        let tmp = std::env::temp_dir().join("mod_maps_test_discovery");
        std::fs::create_dir_all(&tmp).expect("create tmp dir");
        let pak = write_test_pak(
            &tmp,
            "../../../TBL/Content/",
            &[
                ("Mods/TestMod/Maps/MyMap.umap", b"fake"),
                ("Mods/TestMod/Blueprints/Widget.uasset", b"fake"),
                ("SomeGame/Content/Maps/GameMap.umap", b"fake"),
            ],
        );

        let scan = scan_mod_maps(MapSource::PakFiles(vec![pak]), None).expect("scan ok");
        assert_eq!(scan.skipped, Vec::<(String, String)>::new());

        let packages: Vec<&str> = scan.maps.iter().map(|m| m.package_path.as_str()).collect();
        assert_eq!(packages, ["/Game/Mods/TestMod/Maps/MyMap"]);
        assert_eq!(
            scan.maps[0].file_path,
            "../../../TBL/Content/Mods/TestMod/Maps/MyMap.umap"
        );

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn non_umap_and_out_of_scope_entries_are_filtered() {
        let tmp = std::env::temp_dir().join("mod_maps_test_filtered");
        std::fs::create_dir_all(&tmp).expect("create tmp dir");
        let pak = write_test_pak(
            &tmp,
            "../../../TBL/Content/",
            &[
                ("Mods/TestMod/Textures/T.uasset", b"fake"),
                ("Maps/Frontend.umap", b"fake"),
            ],
        );

        let scan = scan_mod_maps(MapSource::PakFiles(vec![pak]), None).expect("scan ok");
        assert!(scan.maps.is_empty());

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn entries_without_content_root_are_skipped_not_registered() {
        let tmp = std::env::temp_dir().join("mod_maps_test_no_content");
        std::fs::create_dir_all(&tmp).expect("create tmp dir");
        let pak = write_test_pak(
            &tmp,
            "../../../Unchained/Mods/",
            &[("Foo/Maps/Bar.umap", b"fake")],
        );

        let scan = scan_mod_maps(MapSource::PakFiles(vec![pak]), None).expect("scan ok");
        assert!(scan.maps.is_empty(), "no package name may be derived without a content root");

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn unparseable_paks_are_skipped_with_a_reason() {
        let tmp = std::env::temp_dir().join("mod_maps_test_garbage");
        std::fs::create_dir_all(&tmp).expect("create tmp dir");
        let pak = tmp.join("garbage.pak");
        std::fs::write(&pak, b"definitely not a pak file").expect("write garbage");

        let scan = scan_mod_maps(MapSource::PakFiles(vec![pak]), None).expect("scan ok");
        assert!(scan.maps.is_empty());
        assert_eq!(scan.skipped.len(), 1, "garbage pak must be reported as skipped");

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn game_pak_names_are_guarded_in_pak_dirs() {
        let tmp = std::env::temp_dir().join("mod_maps_test_guard");
        std::fs::create_dir_all(&tmp).expect("create tmp dir");
        std::fs::write(tmp.join("pakchunk0-WindowsNoEditor.pak"), b"game pak").unwrap();

        let paks = list_pak_files(&tmp).expect("list ok");
        assert!(paks.is_empty(), "pakchunk* must never be scanned");

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn path_derivation_handles_windows_separators_and_case() {
        assert_eq!(
            physical_path_to_package_path(
                "..\\..\\..\\TBL\\Content\\Mods\\Mod A\\Maps\\MyMap.UMAP"
            ),
            None,
            "spaces and dots are invalid in long package names"
        );
        assert_eq!(
            physical_path_to_package_path("../../../TBL/Content/Mods/ModName/Maps/MyMap.umap")
                .as_deref(),
            Some("/Game/Mods/ModName/Maps/MyMap")
        );
        assert_eq!(physical_path_to_package_path("../../../TBL/NotContent/Maps/MyMap.umap"), None);
    }

    #[test]
    fn aes_keys_decode_from_base64_and_hex() {
        assert!(decode_aes_key("0x00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff").is_ok());
        assert!(decode_aes_key("ABEiM0RVZneImaq7zN3E/wASIjNERVZ2iJmqu8zdxEc=").is_ok());
        assert!(decode_aes_key("00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff").is_ok());
        assert!(decode_aes_key("not a key").is_err());
        assert!(decode_aes_key("0x00").is_err(), "wrong length must be rejected");
    }

    /// Smoke test against real paks: run with
    /// `MOD_PAKS_DIR=/mnt/u/Games/Chivalry2_c/TBL/Content/Paks cargo test real_paks_smoke`
    /// (plain mod paks need no key — `repak list` works on them as-is).
    #[test]
    fn real_paks_smoke_when_mod_paks_dir_is_set() {
        let Ok(dir) = std::env::var("MOD_PAKS_DIR") else {
            return; // Not set — hermetic run, nothing to do.
        };
        let scan = scan_mod_maps(MapSource::PakDir(PathBuf::from(dir)), None).expect("scan ok");

        println!("discovered {} mod map(s), skipped {} pak(s)", scan.maps.len(), scan.skipped.len());
        for (pak, reason) in &scan.skipped {
            println!("  skipped: {pak} — {reason}");
        }
        for map in &scan.maps {
            println!("  [map] {}  <-  {}", map.package_path, map.file_path);
        }

        for map in &scan.maps {
            assert!(map.package_path.starts_with(MOD_ROOT), "all maps must be under {MOD_ROOT}");
            assert!(map.file_path.ends_with(".umap"), "file paths keep the .umap extension");
        }
        assert!(
            !scan.maps.is_empty() || !scan.skipped.is_empty(),
            "scanning a real pak dir should find maps or report skips"
        );
    }
}
