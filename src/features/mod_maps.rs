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
//! the game thread (see `game::engine::get_assets_by_path`,
//! `game::engine::scan_files_synchronous`, and the
//! `ScanPathsAndFilesSynchronous` resolver work).
//!
//! NOTE: the `repak` crate is pulled from https://github.com/trumank/repak —
//! the crates.io crate of that name is an unrelated name-squatter.

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

use aes::cipher::KeyInit;
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
    let _scan_started = std::time::Instant::now();
    swarn!(fn; "[modmaps] discover: scan_mod_maps STARTED");
    let pak_paths = match source {
        MapSource::PakDir(dir) => list_pak_files(&dir)?,
        MapSource::PakFiles(files) => files,
    };
    swarn!(fn; "[modmaps] discover: listed {} pak file(s) in {:?}", pak_paths.len(), _scan_started.elapsed());

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
                swarn!(fn; "[modmaps] {}: {} map(s)", name, found);
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
/// characters (callers log these instead of registering garbage); packages
/// outside [`MOD_ROOT`] still derive and are filtered by the caller.
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
// Registration (game thread, amortized)
// ---------------------------------------------------------------------------

/// Max number of package files registered per game-thread tick. Matches the
/// `--mod-map-batch` CLI default; larger batches trade per-tick game-thread time
/// for fewer ticks. The engine's own amortization budget is ~16, so 128 batches
/// do more work per frame.
pub const REGISTRATION_CHUNK_SIZE: usize = 128;

pub type OnRegistrationDone = Box<dyn FnOnce(Result<usize, String>) + Send>;

/// Splits package file paths into game-thread registration batches. Pure logic.
pub fn registration_batches(file_paths: &[String], chunk_size: usize) -> Vec<Vec<String>> {
    file_paths
        .chunks(chunk_size)
        .map(<[String]>::to_vec)
        .collect()
}

/// Registers the given package files into the live Asset Registry, one batch
/// per game-thread tick (each job re-queues the next — the tick hook drains
/// the whole job queue in one frame, so batching must happen from inside the
/// jobs). The completion callback fires on the game thread.
///
/// `chunk_size` is the number of files handed to the engine per tick (see the
/// `--mod-map-batch` CLI knob; the engine's own budget is ~16).
///
/// Safe to call from any thread: only the jobs touch the engine, and each
/// returns `Err` gracefully while the signature is a placeholder.
pub fn register_scanned_mod_maps_async(file_paths: Vec<String>, chunk_size: usize, on_complete: OnRegistrationDone) {
    let total = file_paths.len();
    swarn!(fn; "[modmaps] sending {} file(s) to the Asset Registry", total);
    let batches = VecDeque::from(registration_batches(&file_paths, chunk_size));
    queue_next_registration_batch(batches, 0, total, on_complete);
}

/// Glues discovery and registration: discovers mod maps for `source` on a
/// worker thread, then registers them chunked on the game thread. The
/// completion callback fires on the game thread with the number of
/// registered packages (or the failure reason).
pub fn register_mod_maps_async(source: MapSource, aes_key: Option<String>, on_complete: OnRegistrationDone) {
    let started = std::time::Instant::now();
    discover_mod_maps_async(source, aes_key, Box::new(move |result| {
        match result {
            Ok(scan) => {
                let gather_elapsed = started.elapsed();
                let file_paths: Vec<String> = scan.maps.iter().map(|p| p.file_path.clone()).collect();
                sinfo!(fn; "Discovered {} mod map(s); registering", file_paths.len());
                // Measure the Asset Registry registration step separately from
                // the gathering (pak scan) step.
                let ar_started = std::time::Instant::now();
                register_scanned_mod_maps_async(file_paths, crate::cli_args().mod_map_batch, Box::new(move |r| {
                    let ar_elapsed = ar_started.elapsed();
                    swarn!(fn; "[modmaps] profiler: gathering (pak scan) {:?} | AR registration {:?} | total {:?}", gather_elapsed, ar_elapsed, started.elapsed());
                    on_complete(r);
                }));
            }
            Err(e) => on_complete(Err(e.to_string())),
        }
    }))
}

// ---------------------------------------------------------------------------
// All-assets discovery (benchmark: register every pak entry, no filtering)
// ---------------------------------------------------------------------------

pub type OnAllAssetsDone = Box<dyn FnOnce(Result<AllAssetScan>) + Send>;

/// One discovered asset file: the physical-relative pak path (mount_point +
/// entry) that UE maps back to a package name during registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredAsset {
    pub file_path: String,
    /// Engine package path when derivable (e.g. `/Game/Mods/...`); `None` for
    /// entries that can't be mapped to a package (non-Content paths).
    pub package_path: Option<String>,
}

/// Result of an all-assets discovery pass (every entry, any extension).
#[derive(Debug, Clone, Default)]
pub struct AllAssetScan {
    pub assets: Vec<DiscoveredAsset>,
    pub skipped: Vec<(String, String)>,
}

/// Reads one pak index and collects every entry — no `.umap` or `/Game/Mods`
/// filtering (raw benchmark of "load everything").
fn read_pak_all_assets(pak_path: &Path, key: Option<&aes::Aes256>) -> Result<Vec<DiscoveredAsset>> {
    let mut file = std::fs::File::open(pak_path)
        .with_context(|| format!("failed to open {}", pak_path.display()))?;
    let builder = repak::PakBuilder::new();
    let builder = match key {
        Some(k) => builder.key(k.clone()),
        None => builder,
    };
    let reader = builder.reader(&mut file).context("failed to parse pak index")?;
    let mount_point = reader.mount_point();
    let mut assets = Vec::new();
    for entry in reader.files() {
        let full_path = format!("{}{}", mount_point, entry);
        let package_path = physical_path_to_package_path(&full_path);
        assets.push(DiscoveredAsset { file_path: full_path, package_path });
    }
    Ok(assets)
}

/// Synchronous discovery of every asset in the source paks (no filtering).
pub fn scan_all_assets(source: MapSource, aes_key: Option<String>) -> Result<AllAssetScan> {
    let pak_paths = match source {
        MapSource::PakDir(dir) => list_pak_files(&dir)?,
        MapSource::PakFiles(files) => files,
    };
    let key = aes_key.map(|k| decode_aes_key(&k)).transpose()?;
    let mut scan = AllAssetScan::default();
    let mut seen: HashSet<String> = HashSet::new();
    for pak_path in pak_paths {
        let name = pak_path.display().to_string();
        match read_pak_all_assets(&pak_path, key.as_ref()) {
            Ok(assets) => {
                let mut found = 0usize;
                for a in assets {
                    // Dedup on the engine package path (consistent with the map
                    // path below) so an asset present in several mounted paks is
                    // registered only once. Fall back to the raw file path for
                    // entries that don't map to a package.
                    let key = a.package_path.as_deref().unwrap_or(&a.file_path);
                    if seen.insert(key.to_string()) {
                        scan.assets.push(a);
                        found += 1;
                    }
                }
                swarn!(fn; "[modmaps] {}: {} asset(s)", name, found);
            }
            Err(e) => {
                swarn!(fn; "Skipping pak {}: {}", name, e);
                scan.skipped.push((name, e.to_string()));
            }
        }
    }
    Ok(scan)
}

/// Runs an all-assets discovery pass on a worker thread and delivers the result.
pub fn discover_all_assets_async(source: MapSource, aes_key: Option<String>, on_done: OnAllAssetsDone) {
    crate::features::tokio_runtime::TOKIO_RUNTIME.spawn(async move {
        let joined = tokio::task::spawn_blocking(move || scan_all_assets(source, aes_key)).await;
        on_done(joined
            .context("all-assets scan task panicked or was cancelled")
            .and_then(|r| r));
    });
}

/// Glues all-assets discovery and registration, with gathering/registration/total
/// profiling (mirrors `register_mod_maps_async`).
pub fn register_all_assets_async(source: MapSource, aes_key: Option<String>, on_complete: OnRegistrationDone) {
    let started = std::time::Instant::now();
    discover_all_assets_async(source, aes_key, Box::new(move |result| {
        match result {
            Ok(scan) => {
                let gather_elapsed = started.elapsed();
                let file_paths: Vec<String> = scan.assets.iter().map(|a| a.file_path.clone()).collect();
                sinfo!(fn; "Discovered {} asset(s); registering", file_paths.len());
                let ar_started = std::time::Instant::now();
                register_scanned_mod_maps_async(file_paths, crate::cli_args().mod_map_batch, Box::new(move |r| {
                    let ar_elapsed = ar_started.elapsed();
                    swarn!(fn; "[modmaps] profiler(all-assets): gathering {:?} | AR registration {:?} | total {:?}", gather_elapsed, ar_elapsed, started.elapsed());
                    on_complete(r);
                }));
            }
            Err(e) => on_complete(Err(e.to_string())),
        }
    }))
}

static AUTO_ALL_ASSETS_STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Synchronous, game-thread all-assets registration: runs the pak scan inline
/// (pure Rust, no engine calls) and then every registration batch back-to-back
/// in one go. The Asset Registry work all happens in a single game-thread
/// block, so it is hidden inside the loading screen instead of hitching
/// gameplay one batch per tick.
///
/// MUST run on the game thread (it drives `ScanPathsAndFilesSynchronous`).
/// Intended for the auto/init path (`LoadFrontEndMap` / dedicated-server
/// fallback), where a multi-second block is acceptable. The manual
/// `loadallassets` command keeps the async/chunked path instead.
pub fn register_all_assets_sync() {
    let started = std::time::Instant::now();

    let Some(source) = default_paks_dir().map(MapSource::PakDir) else {
        serror!(fn; "Synchronous all-asset registration skipped: could not derive Content/Paks from the executable");
        return;
    };

    // Gathering step — inline pak index scan (no engine calls).
    let gather_started = std::time::Instant::now();
    let scan = match scan_all_assets(source, None) {
        Ok(scan) => scan,
        Err(e) => {
            serror!(fn; "Synchronous all-asset scan failed: {}", e);
            return;
        }
    };
    let gather_elapsed = gather_started.elapsed();

    let file_paths: Vec<String> = scan.assets.iter().map(|a| a.file_path.clone()).collect();
    let total = file_paths.len();
    sinfo!(fn; "Discovered {} asset(s); registering synchronously", total);

    // Registration step — every batch back-to-back on the game thread.
    let ar_started = std::time::Instant::now();
    let chunk = crate::cli_args().mod_map_batch;
    let mut registered = 0usize;
    for (i, batch) in registration_batches(&file_paths, chunk).iter().enumerate() {
        let batch_timer = std::time::Instant::now();
        if let Err(e) = unsafe { crate::game::engine::scan_files_synchronous(batch) } {
            serror!(fn; "Synchronous registration batch {} failed: {}", i, e);
            return;
        }
        registered += batch.len();
        swarn!(fn; "[modmaps] batch of {} file(s) took {:?}", batch.len(), batch_timer.elapsed());
        sinfo!(fn; "Registered {} package file(s) ({} / {})", batch.len(), registered, total);
    }
    let ar_elapsed = ar_started.elapsed();

    swarn!(fn; "[modmaps] profiler(sync all-assets): gathering {:?} | AR registration {:?} | total {:?}", gather_elapsed, ar_elapsed, started.elapsed());
    sinfo!(fn; "Auto-registered {} asset(s) into the Asset Registry", registered);
}

/// One-shot auto-registration of every asset from the default Content/Paks into
/// the Asset Registry, fired after the mod paks are mounted (from
/// `LoadFrontEndMap`, with a server `world_init` fallback). Runs synchronously
/// on the game thread so the work lands during initialization/loading rather
/// than hitching gameplay; the internal guard makes the call idempotent across
/// call sites, so only the first invocation does any work.
pub fn auto_register_all_assets() {
    use std::sync::atomic::Ordering;
    if AUTO_ALL_ASSETS_STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    register_all_assets_sync();
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

fn queue_next_registration_batch(
    mut remaining: VecDeque<Vec<String>>,
    registered: usize,
    total: usize,
    on_complete: OnRegistrationDone,
) {
    let Some(batch) = remaining.pop_front() else {
        if registered == 0 && total == 0 {
            // Nothing to do — still deliver completion on the game thread.
            crate::resolvers::unchained_integration::run_on_game_thread(move || {
                on_complete(Ok(0));
            });
        } else {
            // Already inside a game-thread job (the last batch re-queued us).
            on_complete(Ok(registered));
        }
        return;
    };
    let next_remaining = remaining;
    let next_registered = registered + batch.len();
    let batch_len = batch.len();
    crate::resolvers::unchained_integration::run_on_game_thread(move || {
        let batch_started = std::time::Instant::now();
        let result = unsafe { crate::game::engine::scan_files_synchronous(&batch) };
        if let Err(e) = result {
            serror!(fn; "Registration batch failed: {}", e);
            on_complete(Err(e));
            return;
        }
        swarn!(fn; "[modmaps] batch of {} file(s) took {:?}", batch_len, batch_started.elapsed());
        sinfo!(fn; "Registered {} package file(s) ({} / {})", batch_len, next_registered, total);
        queue_next_registration_batch(next_remaining, next_registered, total, on_complete);
    });
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
    fn scans_committed_dummy_test_pak() {
        // The dummy v9 pak is generated with repak and committed under
        // test-resources (4 entries: 2 mod maps, 1 mod asset, 1 game map).
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let pak_path = Path::new(manifest_dir).join("test-resources/mod_maps/test_mod.pak");
        if !pak_path.exists() {
            eprintln!("skipping: {} not present in this crate", pak_path.display());
            return;
        }

        let scan = scan_mod_maps(MapSource::PakFiles(vec![pak_path]), None).expect("scan ok");
        let packages: Vec<&str> = scan.maps.iter().map(|m| m.package_path.as_str()).collect();
        assert_eq!(packages, [
            "/Game/Mods/EvalMod/Maps/EvalMap",
            "/Game/Mods/EvalMod/Maps/Nested/InnerMap",
        ]);
    }

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
