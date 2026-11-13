//! Mod asset discovery from the loader's mounted mod `.pak` files, and their
//! registration into the Asset Registry at startup.
//!
//! Discovery is pure Rust: pak indexes are read with `repak`, every `.umap` and
//! every `.uasset` holding a marker instance is kept, as the physical-relative
//! pak path (`mount_point + entry`) that UE's
//! `FPackageName::FilenameToLongPackageName` maps back to the package name
//! during registration. Paks that ship a `ModAssetRegistry.bin` are merged
//! natively instead.
//!
//! Only `register_filtered_assets_sync` and what it calls touch the engine, and
//! must run on the game thread.
//!
//! NOTE: the `repak` crate is pulled from https://github.com/trumank/repak —
//! the crates.io crate of that name is an unrelated name-squatter.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use aes::cipher::KeyInit;
use anyhow::{Context, Result};

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
            sdebug!( "Skipping game pak {}", path.display());
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
/// characters, and for anything that is not a `.umap`.
fn physical_path_to_package_path(full_path: &str) -> Option<String> {
    let normalized = full_path.replace('\\', "/");
    let content_start = normalized.find(CONTENT_SEGMENT)? + CONTENT_SEGMENT.len();
    let remainder = normalized.get(content_start..)?;
    let stem = strip_umap(remainder)?;
    let package_path = format!("/Game/{}", stem);
    if !package_path.contains("//")
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
/// matching the repak CLI convention. Base64 results are only accepted when
/// they decode to exactly 32 bytes, so plain-hex keys (which are also valid
/// base64 strings) fall through to the hex path.
fn decode_aes_key(key: &str) -> Result<aes::Aes256> {
    let trimmed = key.trim();
    if let Some(hex_part) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        let bytes = hex::decode(hex_part).context("invalid hex AES key")?;
        return aes::Aes256::new_from_slice(&bytes).context("AES key must decode to 32 bytes");
    }
    use base64::Engine as _;
    let bytes = match base64::engine::general_purpose::STANDARD
        .decode(trimmed)
        .ok()
        .filter(|bytes| bytes.len() == 32)
    {
        Some(bytes) => bytes,
        None => hex::decode(trimmed).context("AES key must be base64 or hex")?,
    };
    aes::Aes256::new_from_slice(&bytes).context("AES key must decode to 32 bytes")
}

// ---------------------------------------------------------------------------
// Registration (game thread)
// ---------------------------------------------------------------------------

/// Splits package file paths into registration batches of `chunk_size` files
/// (`--mod-map-batch`, default 128), one engine call each. Pure logic.
pub fn registration_batches(file_paths: &[String], chunk_size: usize) -> Vec<Vec<String>> {
    // `chunks` panics on 0 (`--mod-map-batch 0`); treat it as 1.
    file_paths
        .chunks(chunk_size.max(1))
        .map(<[String]>::to_vec)
        .collect()
}

/// One discovered asset file: the physical-relative pak path (mount_point +
/// entry) that UE maps back to a package name during registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredAsset {
    pub file_path: String,
    /// Engine package path when derivable (e.g. `/Game/Mods/...`); `None` for
    /// entries that can't be mapped to a package (non-Content paths).
    pub package_path: Option<String>,
}

// ---------------------------------------------------------------------------
// Filtered discovery (maps + marker packages) — the cheap main-menu path
// ---------------------------------------------------------------------------

/// Per-scan timing split, so we can see how long finding the pak assets costs
/// vs how long the header parse ("finding markers") costs.
#[derive(Debug, Clone, Copy, Default)]
pub struct FilterStats {
    /// Time spent reading pak indexes (enumerating entries).
    pub index_elapsed: std::time::Duration,
    /// Time spent parsing package headers to detect marker instances.
    pub parse_elapsed: std::time::Duration,
    /// Total files enumerated across all scanned paks (any extension).
    pub total_entries: usize,
    /// Total `.uasset` files seen across all scanned paks.
    pub total_uasset: usize,
    /// Number of paks scanned.
    pub total_paks: usize,
    /// Number of worker threads used to scan the paks (parallelism).
    pub threads: usize,
}

/// Result of the filtered discovery pass (maps + marker data-asset packages).
#[derive(Debug, Clone, Default)]
pub struct FilteredScan {
    pub assets: Vec<DiscoveredAsset>,
    pub skipped: Vec<(String, String)>,
    /// Number of `.uasset` headers parsed.
    pub parsed_packages: usize,
    /// Per-category package counts (maps and each marker kind).
    pub counts: CategoryCounts,
    pub stats: FilterStats,
}

/// Counts of the different package kinds the filtered path picks up.
#[derive(Debug, Clone, Copy, Default)]
pub struct CategoryCounts {
    /// `.umap` map packages.
    pub maps: usize,
    /// Packages containing a `DA_ModMarker` (V1) instance.
    pub mod_markers: usize,
    /// Packages containing a `DA_ModMarker_V2` instance.
    pub mod_markers_v2: usize,
    /// Packages containing a `DA_MapMarker_V2` instance.
    pub map_markers: usize,
    /// Packages containing a `DA_MapInfo` instance.
    pub map_infos: usize,
}

/// Reads one pak: keeps every `.umap` (map) plus every `.uasset` package that
/// contains a marker instance (V1/V2 mod marker, map info, map marker; detected
/// via header parse).
/// Other extensions (`.uexp`, `.ubulk`, `.ini`, ...) are skipped.
fn read_pak_filtered_assets(
    pak_path: &Path,
    key: Option<&aes::Aes256>,
    stats: &mut FilterStats,
) -> Result<(Vec<DiscoveredAsset>, usize, CategoryCounts)> {
    let mut file = std::fs::File::open(pak_path)
        .with_context(|| format!("failed to open {}", pak_path.display()))?;
    let builder = repak::PakBuilder::new();
    let builder = match key {
        Some(k) => builder.key(k.clone()),
        None => builder,
    };
    let mut br = std::io::BufReader::new(&mut file);
    let reader = builder.reader(&mut br).context("failed to parse pak index")?;
    let mount_point = reader.mount_point().to_string();

    let index_t = std::time::Instant::now();
    let entries = reader.files();
    stats.index_elapsed += index_t.elapsed();
    stats.total_paks += 1;
    stats.total_entries += entries.len();

    let mut assets = Vec::new();
    let mut parsed = 0usize;
    let mut counts = CategoryCounts::default();
    for entry in entries {
        let lower = entry.to_lowercase();
        let full_path = format!("{}{}", mount_point, entry);
        if lower.ends_with(".umap") {
            // Maps are always wanted; no header read needed.
            let package_path = physical_path_to_package_path(&full_path);
            counts.maps += 1;
            assets.push(DiscoveredAsset { file_path: full_path, package_path });
        } else if lower.ends_with(".uasset") {
            // Read the package header and keep it only if it holds a marker instance.
            let package_path = physical_path_to_package_path(&full_path);
            stats.total_uasset += 1;
            parsed += 1;
            let parse_t = std::time::Instant::now();
            let kind = reader
                .get(&entry, &mut br)
                .map(|data| crate::features::cooked_header::marker_counts(&data))
                .unwrap_or_default();
            stats.parse_elapsed += parse_t.elapsed();
            if kind.any() {
                counts.mod_markers += usize::from(kind.mod_markers_v1 > 0);
                counts.mod_markers_v2 += usize::from(kind.mod_markers_v2 > 0);
                counts.map_infos += usize::from(kind.map_infos > 0);
                counts.map_markers += usize::from(kind.map_markers_v2 > 0);
                assets.push(DiscoveredAsset { file_path: full_path, package_path });
            }
        }
        // Non-package extensions are skipped.
    }
    Ok((assets, parsed, counts))
}

/// Read a namespaced runtime registry payload from one mod pak. This is pure
/// Rust/pak I/O; the caller must hand the bytes to the engine only on the game
/// thread. Project-root AssetRegistry.bin is intentionally never considered.
fn read_pak_mod_asset_registry(pak_path: &Path, key: Option<&aes::Aes256>) -> Result<Option<Vec<u8>>> {
    let mut file = std::fs::File::open(pak_path)
        .with_context(|| format!("failed to open {}", pak_path.display()))?;
    let builder = match key {
        Some(key) => repak::PakBuilder::new().key(key.clone()),
        None => repak::PakBuilder::new(),
    };
    let reader = builder.reader(&mut file).context("failed to parse pak index")?;
    let mount_point = reader.mount_point().to_string();
    let entry = reader.files().into_iter().find(|entry| {
        if !entry
            .rsplit('/')
            .next()
            .map(|name| name.eq_ignore_ascii_case(crate::features::mod_asset_registry::MOD_ASSET_REGISTRY_FILENAME))
            .unwrap_or(false)
        {
            return false;
        }
        let full_path = format!("{}{}", mount_point, entry);
        let normalized = full_path.replace('\\', "/");
        normalized
            .find(CONTENT_SEGMENT)
            .and_then(|start| normalized.get(start + CONTENT_SEGMENT.len()..))
            .map(|path| path.starts_with("Mods/"))
            .unwrap_or(false)
    });
    match entry {
        Some(entry) => reader
            .get(&entry, &mut file)
            .map(Some)
            .with_context(|| format!("failed to read {} from {}", entry, pak_path.display())),
        None => Ok(None),
    }
}

/// Partition paks into the mutually exclusive native-registry and legacy
/// discovery paths. A pak whose optional registry cannot be inspected is
/// skipped rather than falling through to discovery, avoiding double work if
/// the entry was present but unreadable.
fn classify_paks_for_asset_registry(paks: Vec<PathBuf>) -> (Vec<(PathBuf, Vec<u8>)>, Vec<PathBuf>) {
    let mut registry_paks = Vec::new();
    let mut legacy_paks = Vec::new();
    for pak_path in paks {
        match read_pak_mod_asset_registry(&pak_path, None) {
            Ok(Some(bytes)) => registry_paks.push((pak_path, bytes)),
            Ok(None) => legacy_paks.push(pak_path),
            Err(error) => serror!("[modregistry] failed to inspect {}; skipping pak: {}", pak_path.display(), error),
        }
    }
    (registry_paks, legacy_paks)
}

/// Synchronously merge the registry-bearing half of a classified pak set with
/// `merge` (the native merge in production; must then run on the game thread).
/// Returns the number of merged paks and the paks whose merge failed, so the
/// caller can send those through legacy header discovery instead of losing
/// their maps and markers.
fn merge_mod_asset_registries_sync<E: std::fmt::Display>(
    registry_paks: Vec<(PathBuf, Vec<u8>)>,
    mut merge: impl FnMut(&[u8]) -> Result<(), E>,
) -> (usize, Vec<PathBuf>) {
    let mut merged = 0usize;
    let mut failed = Vec::new();
    for (pak_path, bytes) in registry_paks {
        match merge(&bytes) {
            Ok(()) => {
                merged += 1;
                // sdebug!("[modregistry] merged {} from {}", crate::features::mod_asset_registry::MOD_ASSET_REGISTRY_FILENAME, pak_path.display());
            }
            Err(error) => {
                serror!("[modregistry] failed to merge {}: {}", pak_path.display(), error);
                failed.push(pak_path);
            }
        }
    }
    (merged, failed)
}

/// Synchronous discovery of maps + marker packages across the source paks.
/// Paks are processed in parallel across worker threads (pure Rust: repak index
/// reads + header parse; no engine calls), then aggregated in deterministic
/// (original pak) order. Results are identical to a sequential scan.
pub fn scan_filtered_assets(source: MapSource, aes_key: Option<String>) -> Result<FilteredScan> {
    let pak_paths = match source {
        MapSource::PakDir(dir) => list_pak_files(&dir)?,
        MapSource::PakFiles(files) => files,
    };
    let key = aes_key.map(|k| decode_aes_key(&k)).transpose()?;

    let n = pak_paths.len();
    if n == 0 {
        return Ok(FilteredScan::default());
    }
    // Pak scanning is IO-bound; a modest worker count beats max oversubscription
    // (measured: 12 threads slower than 4 on this disk). Cap at 8.
    let num_threads = std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(4)
        .min(8)
        .min(n)
        .max(1);

    // Each worker handles every `num_threads`th pak (strided interleave) and
    // returns (pak_index, result, per-pak stats).
    let results: Vec<(usize, Result<(Vec<DiscoveredAsset>, usize, CategoryCounts), String>, FilterStats)> =
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..num_threads)
                .map(|t| {
                    let pak_paths = &pak_paths;
                    let key = &key;
                    scope.spawn(move || {
                        let mut out = Vec::new();
                        let mut i = t;
                        while i < pak_paths.len() {
                            let mut stats = FilterStats::default();
                            // A panic while reading one pak (third-party data)
                            // only skips that pak.
                            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                read_pak_filtered_assets(&pak_paths[i], key.as_ref(), &mut stats)
                            }))
                            .unwrap_or_else(|_| Err(anyhow::anyhow!("pak scan panicked")));
                            out.push((i, r.map_err(|e| e.to_string()), stats));
                            i += num_threads;
                        }
                        out
                    })
                })
                .collect();
            let mut all = Vec::new();
            for (t, h) in handles.into_iter().enumerate() {
                match h.join() {
                    Ok(out) => all.extend(out),
                    // Report every pak of a dead worker as skipped.
                    Err(_) => all.extend(
                        (t..pak_paths.len())
                            .step_by(num_threads)
                            .map(|i| (i, Err("pak scan worker panicked".to_string()), FilterStats::default())),
                    ),
                }
            }
            all
        });

    let mut scan = FilteredScan::default();
    scan.stats.threads = num_threads;
    let mut seen: HashSet<String> = HashSet::new();
    let mut by_index: Vec<&(usize, Result<(Vec<DiscoveredAsset>, usize, CategoryCounts), String>, FilterStats)> =
        results.iter().collect();
    by_index.sort_by_key(|r| r.0);
    for (idx, result, stats) in by_index {
        scan.stats.total_entries += stats.total_entries;
        scan.stats.total_uasset += stats.total_uasset;
        scan.stats.total_paks += stats.total_paks;
        let name = pak_paths[*idx].display().to_string();
        match result {
            Ok((assets, parsed, counts)) => {
                scan.parsed_packages += parsed;
                scan.counts.maps += counts.maps;
                scan.counts.mod_markers += counts.mod_markers;
                scan.counts.map_infos += counts.map_infos;
                scan.counts.mod_markers_v2 += counts.mod_markers_v2;
                scan.counts.map_markers += counts.map_markers;
                for a in assets {
                    let k = a.package_path.as_deref().unwrap_or(&a.file_path);
                    if seen.insert(k.to_string()) {
                        scan.assets.push(a.clone());
                    }
                }
            }
            Err(e) => {
                sdebug!( "Skipping pak {}: {}", name, e);
                scan.skipped.push((name, e.clone()));
            }
        }
    }
    Ok(scan)
}


static AUTO_ALL_ASSETS_STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// One-shot auto-registration of maps + marker packages into the Asset Registry,
/// fired after the mod paks are mounted (from `LoadFrontEndMap`, with a server
/// `world_init` fallback). Uses the cheap filtered path (blanket all-assets or
/// maps-only commands are still available explicitly). Runs synchronously on the
/// game thread during init; the internal guard makes the call idempotent.
pub fn auto_register_all_assets() {
    use std::sync::atomic::Ordering;
    if AUTO_ALL_ASSETS_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    register_filtered_assets_sync();
    log_registered_markers();
    // Reading the map markers loads their assets, so it runs as its own game
    // thread job on a later tick rather than inside the registration call.
    crate::resolvers::unchained_integration::run_on_game_thread(|| {
        unsafe { crate::features::map_markers::scan_map_markers() };
    });
}

/// Logs how many marker assets the Asset Registry holds after registration,
/// per marker class. This covers both ways in: the header scan and a pak's
/// merged `ModAssetRegistry.bin` (which the header scan never looks at).
///
/// MUST run on the game thread.
fn log_registered_markers() {
    let counts: Vec<String> = ["DA_ModMarker_C", "DA_ModMarker_V2_C", "DA_MapInfo_C", "DA_MapMarker_V2_C"]
        .iter()
        .map(|class_name| {
            // A failed query means the registry has no asset of that class.
            let count = unsafe { crate::game::engine::get_assets_by_class(class_name.to_string()) }
                .map(|assets| assets.len())
                .unwrap_or(0);
            format!("{count} {class_name}")
        })
        .collect();
    sdebug!("[markers] Asset Registry holds {}", counts.join(", "));
}

/// Synchronous, game-thread registration of just the main-menu assets: every
/// map plus every package containing a DA_ModMarker / DA_MapInfo instance. This
/// is the production init path — far cheaper than the all-assets benchmark
/// (dozens of packages vs thousands). All batches run back-to-back on the game
/// thread inside the loading screen.
///
/// MUST run on the game thread (it drives `ScanPathsAndFilesSynchronous`).
pub fn register_filtered_assets_sync() {
    let started = std::time::Instant::now();

    let paks = discovery_pak_paths();
    if paks.is_empty() {
        serror!("Synchronous filtered registration skipped: no mod paks found (mounted list empty and no default Content/Paks)");
        return;
    }
    let (registry_paks, mut legacy_paks) = classify_paks_for_asset_registry(paks);
    let (registry_merged, failed_merges) = merge_mod_asset_registries_sync(registry_paks, |bytes| unsafe {
        crate::features::mod_asset_registry::append_mod_asset_registry_on_game_thread(bytes)
    });
    if crate::cli_args().mod_asset_registry_only {
        sdebug!("[modregistry] registry-only test merged {} pak(s); legacy ScanPathsAndFilesSynchronous was not called", registry_merged);
        return;
    }
    if !failed_merges.is_empty() {
        // A registry pak normally skips the header scan; one whose merge
        // failed would otherwise register nothing at all.
        swarn!("[modregistry] {} pak(s) failed to merge; falling back to header discovery for them", failed_merges.len());
        legacy_paks.extend(failed_merges);
    }
    if legacy_paks.is_empty() {
        sdebug!("[modregistry] merged {} registry pak(s); no registry-less paks require legacy discovery", registry_merged);
        return;
    }
    let source = MapSource::PakFiles(legacy_paks);

    // Gathering + marker detection (pure Rust, no engine calls), parallel.
    let marker_started = std::time::Instant::now();
    let scan = match scan_filtered_assets(source, None) {
        Ok(scan) => scan,
        Err(e) => {
            serror!("Synchronous filtered scan failed: {}", e);
            return;
        }
    };
    let marker_elapsed = marker_started.elapsed();

    let file_paths: Vec<String> = scan.assets.iter().map(|a| a.file_path.clone()).collect();

    // Registration step — every batch back-to-back on the game thread.
    let ar_started = std::time::Instant::now();
    let chunk = crate::cli_args().mod_map_batch;
    let mut registered = 0usize;
    for (i, batch) in registration_batches(&file_paths, chunk).iter().enumerate() {
        if let Err(e) = unsafe { crate::game::engine::scan_files_synchronous(batch) } {
            serror!("Synchronous filtered batch {} failed: {}", i, e);
            return;
        }
        registered += batch.len();
    }
    let ar_elapsed = ar_started.elapsed();

    sdebug!( "[modmaps] registered {} package(s): {} map(s), {} DA_ModMarker, {} DA_ModMarker_V2, {} DA_MapInfo, {} DA_MapMarker_V2 (from {} .uasset across {} pak(s), {} thread(s))",
        registered, scan.counts.maps, scan.counts.mod_markers, scan.counts.mod_markers_v2,
        scan.counts.map_infos, scan.counts.map_markers,
        scan.stats.total_uasset, scan.stats.total_paks, scan.stats.threads);
    sdebug!( "[modmaps] profiler: marker-detection ({}) {:?} | AR registration {:?} | total {:?}",
        scan.stats.threads, marker_elapsed, ar_elapsed, started.elapsed());

    // List the loaded maps so the main-menu package set is easy to verify.
    // let maps: Vec<&DiscoveredAsset> = scan
    //     .assets
    //     .iter()
    //     .filter(|a| a.file_path.to_lowercase().ends_with(".umap"))
    //     .collect();
    // sdebug!( "Loaded {} map(s):", maps.len());
    // for m in &maps {
    //     sdebug!( "  {}", m.package_path.as_deref().unwrap_or(&m.file_path));
    // }
}

/// The game's `Content/Paks` directory, derived from the running executable
/// (`.../TBL/Binaries/Win64/Chivalry2-Win64-Shipping.exe` → `.../TBL/Content/Paks`).
/// This is where the loader's mod paks live; the `pakchunk*` guard excludes the
/// game's own paks.
pub fn default_paks_dir() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|exe| {
        exe.parent() // Binaries/Win64
            .and_then(|p| p.parent()) // Binaries
            .and_then(|p| p.parent()) // <Project>
            .map(|project| project.join("Content").join("Paks"))
    })
}

/// Filter a mounted-pak filename list down to non-game (non-`pakchunk*`) `.pak`
/// files, sorted and deduped. Pure; unit-tested.
fn filter_mounted_paks(mounted: &[String]) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = mounted
        .iter()
        .map(PathBuf::from)
        .filter(|p| {
            p.extension().map(|e| e.eq_ignore_ascii_case("pak")).unwrap_or(false)
                && !p.file_name()
                    .map(|n| n.to_string_lossy().to_lowercase().starts_with("pakchunk"))
                    .unwrap_or(false)
        })
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

/// Candidate pak paths for discovery: the engine's actually-mounted paks (covers
/// extra dirs and excludes unsigned / not-loadable paks), filtered to non-game
/// paks. Falls back to a `Content/Paks` scan if no mounts were captured (e.g. the
/// `Mount` resolver isn't populated yet).
fn discovery_pak_paths() -> Vec<PathBuf> {
    let mounted = crate::resolvers::pak_mount::mounted_pak_filenames();
    let fallback = || {
        default_paks_dir().and_then(|d| list_pak_files(&d).ok()).unwrap_or_default()
    };
    if mounted.is_empty() {
        sdebug!("[modmaps] pak discovery: Mount list empty — falling back to Content/Paks scan");
        return fallback();
    }
    let paths = filter_mounted_paks(&mounted);
    if paths.is_empty() {
        sdebug!("[modmaps] pak discovery: {} mounted, all game (pakchunk) — falling back to Content/Paks scan", mounted.len());
        return fallback();
    }
    // sdebug!("[modmaps] pak discovery: using Mount — {} mod pak(s) loaded:", paths.len());
    // for p in &paths {
    //     sdebug!("[modmaps]   {}", p.display());
    // }
    paths
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
    fn filtered_scan_keeps_maps_and_marker_packages_only() {
        let tmp = std::env::temp_dir().join("mod_maps_test_filtered_scan");
        std::fs::create_dir_all(&tmp).expect("create tmp dir");
        let marker = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("test-resources/cooked_header/NihiMaps_Marker.uasset"),
        )
        .expect("marker fixture");
        let pak = write_test_pak(
            &tmp,
            "../../../TBL/Content/",
            &[
                ("Mods/TestMod/Maps/MyMap.umap", b"fake"),
                ("Mods/TestMod/Blueprints/Widget.uasset", b"fake"),
                ("Mods/TestMod/TestMod_Marker.uasset", &marker),
                ("Mods/TestMod/Maps/MyMap.uexp", b"fake"),
            ],
        );

        let scan = scan_filtered_assets(MapSource::PakFiles(vec![pak]), None).expect("scan ok");
        assert_eq!(scan.skipped, Vec::<(String, String)>::new());

        let mut files: Vec<&str> = scan.assets.iter().map(|a| a.file_path.as_str()).collect();
        files.sort();
        assert_eq!(files, [
            "../../../TBL/Content/Mods/TestMod/Maps/MyMap.umap",
            "../../../TBL/Content/Mods/TestMod/TestMod_Marker.uasset",
        ]);
        let map = scan.assets.iter().find(|a| a.file_path.ends_with(".umap")).unwrap();
        assert_eq!(map.package_path.as_deref(), Some("/Game/Mods/TestMod/Maps/MyMap"));
        assert_eq!(scan.counts.maps, 1);
        assert_eq!(scan.counts.mod_markers, 1);
        assert_eq!(scan.parsed_packages, 2);

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn unparseable_paks_are_skipped_with_a_reason() {
        let tmp = std::env::temp_dir().join("mod_maps_test_garbage");
        std::fs::create_dir_all(&tmp).expect("create tmp dir");
        let pak = tmp.join("garbage.pak");
        std::fs::write(&pak, b"definitely not a pak file").expect("write garbage");

        let scan = scan_filtered_assets(MapSource::PakFiles(vec![pak]), None).expect("scan ok");
        assert!(scan.assets.is_empty());
        assert_eq!(scan.skipped.len(), 1, "garbage pak must be reported as skipped");

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn scans_committed_dummy_test_pak() {
        // The dummy v9 pak is generated with repak and committed under
        // test-resources (4 entries: 2 mod maps, 1 mod asset, 1 game map).
        let pak_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("test-resources/mod_maps/test_mod.pak");
        let scan = scan_filtered_assets(MapSource::PakFiles(vec![pak_path]), None).expect("scan ok");
        let packages: Vec<&str> = scan.assets.iter().filter_map(|a| a.package_path.as_deref()).collect();
        for expected in ["/Game/Mods/EvalMod/Maps/EvalMap", "/Game/Mods/EvalMod/Maps/Nested/InnerMap"] {
            assert!(packages.contains(&expected), "missing {expected} in {packages:?}");
        }
        assert_eq!(scan.counts.maps, 3);
        assert!(scan.assets.iter().all(|a| a.file_path.to_lowercase().ends_with(".umap")));
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
    fn reads_only_namespaced_mod_asset_registry_payload() {
        let tmp = std::env::temp_dir().join("mod_maps_test_asset_registry");
        std::fs::create_dir_all(&tmp).expect("create tmp dir");
        let pak = write_test_pak(
            &tmp,
            "../../../TBL/Content/",
            &[
                ("Mods/TestMod/ModAssetRegistry.bin", b"registry"),
                ("AssetRegistry.bin", b"must-not-read"),
            ],
        );
        let payload = read_pak_mod_asset_registry(&pak, None)
            .expect("read pak")
            .expect("namespaced registry");
        assert_eq!(payload, b"registry");
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn registry_paks_are_excluded_from_legacy_discovery() {
        let tmp = std::env::temp_dir().join("mod_maps_test_registry_routing");
        let with_registry = tmp.join("with_registry");
        let without_registry = tmp.join("without_registry");
        std::fs::create_dir_all(&with_registry).expect("create registry dir");
        std::fs::create_dir_all(&without_registry).expect("create legacy dir");
        let registry_pak = write_test_pak(
            &with_registry,
            "../../../TBL/Content/",
            &[("Mods/RegistryMod/ModAssetRegistry.bin", b"registry")],
        );
        let legacy_pak = write_test_pak(
            &without_registry,
            "../../../TBL/Content/",
            &[("Mods/LegacyMod/Maps/Legacy.umap", b"map")],
        );

        let (registry, legacy) = classify_paks_for_asset_registry(vec![registry_pak.clone(), legacy_pak.clone()]);
        assert_eq!(registry, vec![(registry_pak, b"registry".to_vec())]);
        assert_eq!(legacy, vec![legacy_pak]);
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn failed_registry_merges_are_returned_for_legacy_discovery() {
        let paks = vec![
            (PathBuf::from("good.pak"), b"ok".to_vec()),
            (PathBuf::from("bad.pak"), b"broken".to_vec()),
        ];
        let (merged, failed) = merge_mod_asset_registries_sync(paks, |bytes| {
            if bytes == b"ok" { Ok(()) } else { Err("merge failed") }
        });
        assert_eq!(merged, 1);
        assert_eq!(failed, vec![PathBuf::from("bad.pak")]);
    }

    #[test]
    fn registration_batches_treat_zero_chunk_size_as_one() {
        let files = vec!["a".to_string(), "b".to_string()];
        assert_eq!(registration_batches(&files, 0), vec![vec!["a".to_string()], vec!["b".to_string()]]);
        assert!(registration_batches(&[], 0).is_empty());
    }

    #[test]
    fn registration_batches_chunk_and_preserve_order() {
        let files: Vec<String> = (0..35)
            .map(|i| format!("/Game/Mods/TestMod/Maps/Map{i}.umap"))
            .collect();
        let batches = registration_batches(&files, 16);
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].len(), 16);
        assert_eq!(batches[1].len(), 16);
        assert_eq!(batches[2].len(), 3);
        let flattened: Vec<String> = batches.concat();
        assert_eq!(flattened, files, "batches must preserve order and content");
    }

    #[test]
    fn exact_chunk_boundaries_do_not_spill() {
        let files: Vec<String> = (0..16)
            .map(|i| format!("/Game/Mods/M{i}.umap"))
            .collect();
        let batches = registration_batches(&files, 16);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].len(), 16);

        let empty: Vec<String> = Vec::new();
        assert!(registration_batches(&empty, 16).is_empty());
    }

    #[test]
    fn custom_chunk_size_is_honored() {
        let files: Vec<String> = (0..10)
            .map(|i| format!("/Game/Mods/M{i}.umap"))
            .collect();
        let batches = registration_batches(&files, 4);
        assert_eq!(
            batches.iter().map(|b| b.len()).collect::<Vec<_>>(),
            vec![4, 4, 2]
        );
        // A chunk larger than the total yields a single batch.
        let large = registration_batches(&files, 100);
        assert_eq!(large.len(), 1);
        assert_eq!(large[0].len(), 10);
    }

    #[test]
    fn filter_mounted_paks_excludes_game_paks_and_dedups() {
        let mounted = [
            "U:/Games/Chivalry2_c/TBL/Content/Paks/pakchunk0-WindowsNoEditor.pak".to_string(),
            "U:/Games/Chivalry2_c/TBL/Content/Paks/NihiMaps.pak".to_string(),
            "U:/Games/Chivalry2_c/TBL/Content/Paks/NihiMaps.pak".to_string(),
            "U:/Games/Chivalry2_c/TBL/Content/Paks/Hippodromeum.pak".to_string(),
        ];
        let filtered = filter_mounted_paks(&mounted);
        let names: Vec<String> = filtered
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        // pakchunk excluded; duplicate NihiMaps deduped; sorted.
        assert_eq!(names, vec!["Hippodromeum.pak", "NihiMaps.pak"]);
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

    /// Manual check against real paks: set `UNCHAINED_TEST_PAK_DIR` to a
    /// `Content/Paks` directory and run with `--ignored --nocapture`.
    #[test]
    #[ignore = "needs UNCHAINED_TEST_PAK_DIR pointing at real paks"]
    fn prints_marker_counts_for_a_real_pak_dir() {
        let dir = std::env::var("UNCHAINED_TEST_PAK_DIR").expect("UNCHAINED_TEST_PAK_DIR not set");
        let scan = scan_filtered_assets(MapSource::PakDir(PathBuf::from(dir)), None).expect("scan");
        println!("counts: {:?}", scan.counts);
        println!("skipped paks: {:?}", scan.skipped);
        for asset in scan.assets.iter().filter(|a| !a.file_path.to_lowercase().ends_with(".umap")) {
            println!("marker package: {}", asset.file_path);
        }
    }
}
