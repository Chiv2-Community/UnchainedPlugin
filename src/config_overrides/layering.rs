use itertools::Itertools;
use std::{cell::RefCell, collections::HashMap, ptr::NonNull, sync::atomic::Ordering};

use super::{with_state, GENERATION};
use crate::{
    resolvers::ini::{o_ConfigFileAddLayer, o_ConfigFileCopy, o_ConfigFileDestroy},
    tools::hook_globals::globals_initialized,
    ue::{FConfigCacheIni, FConfigFile, FString, TCHAR},
};

use super::selection::{
    build_override_paths, build_profile_override_path, config_filename, LayerSelection,
};

/// A destination's INI filenames mapped to ordered layer paths, independent of native discovery.
#[derive(Default)]
pub(super) struct ConfigLayers {
    paths: HashMap<String, Vec<String>>,
}

impl ConfigLayers {
    pub fn new(root: &str, layers: &[LayerSelection]) -> Self {
        let mut prepared = Self::default();
        for selection in layers {
            let folder = build_profile_override_path(root, selection.layer, &selection.slot, "");
            let Ok(entries) = std::fs::read_dir(folder) else {
                continue;
            };
            entries
                .flatten()
                .filter(|entry| entry.path().is_file())
                .filter_map(|entry| {
                    let name = entry.file_name().into_string().ok()?;
                    config_filename(&name)?;
                    Some(name)
                })
                .sorted()
                .dedup_by(|a, b| a.eq_ignore_ascii_case(b))
                .for_each(|name| {
                    prepared
                        .paths
                        .entry(name.to_ascii_lowercase())
                        .or_default()
                        .extend(build_override_paths(
                            root,
                            &name,
                            std::slice::from_ref(selection),
                        ));
                });
        }
        prepared
    }

    pub fn paths(&self, filename: &str) -> &[String] {
        config_filename(filename)
            .and_then(|name| self.paths.get(&name.to_ascii_lowercase()))
            .map_or(&[], Vec::as_slice)
    }

    pub fn categories(&self) -> impl Iterator<Item = String> + '_ {
        self.paths
            .keys()
            .map(|name| name[..name.len() - 4].to_owned())
    }
}

struct OwnedConfig {
    // Native callers retain writable pointers; never reborrow the allocation through &FConfigFile.
    file: NonNull<FConfigFile>,
}

impl OwnedConfig {
    unsafe fn copy(source: *const FConfigFile) -> Self {
        let file =
            NonNull::new_unchecked(Box::into_raw(Box::new(std::mem::zeroed::<FConfigFile>())));
        CALL_ORIGINAL!(ConfigFileCopy(file.as_ptr(), source));
        // Do not mutate or destroy the baseline's source config through the shallow copy.
        (*file.as_ptr()).source_config_file = std::ptr::null_mut();
        (*file.as_ptr()).no_save = 1;
        Self { file }
    }

    fn ptr(&self) -> *mut FConfigFile {
        self.file.as_ptr()
    }
}

impl Drop for OwnedConfig {
    fn drop(&mut self) {
        CALL_ORIGINAL!(ConfigFileDestroy(self.ptr()));
        // Native destruction releases UE-owned members, not this Rust-owned outer allocation.
        unsafe { drop(Box::from_raw(self.file.as_ptr())) };
    }
}

#[derive(Default)]
struct OverlayCache {
    generation: u64,
    // Small per-thread routing table, including configs without conditional layers.
    files: Vec<Overlay>,
}

struct OverlayKey {
    cache_address: usize,
    filename: Box<[TCHAR]>,
    baseline_address: usize,
}

struct Overlay {
    key: OverlayKey,
    file: Option<OwnedConfig>,
}

impl OverlayCache {
    fn retire(&mut self, generation: u64) -> Vec<Overlay> {
        if self.generation == generation {
            return Vec::new();
        }
        self.generation = generation;
        std::mem::take(&mut self.files)
    }

    fn find(
        &self,
        cache: *mut FConfigCacheIni,
        filename: &[TCHAR],
        baseline: *mut FConfigFile,
    ) -> Option<*mut FConfigFile> {
        self.files
            .iter()
            .find(|entry| {
                entry.key.cache_address == cache as usize
                    && entry.key.filename.as_ref() == filename
                    && entry.key.baseline_address == baseline as usize
            })
            .map(|entry| entry.file.as_ref().map_or(baseline, OwnedConfig::ptr))
    }
}

thread_local! {
    // UE returns borrowed section/file pointers. Per-thread copies avoid freeing a reader's
    // storage when another thread changes map context. No RefCell borrow spans a native call.
    static CACHE: RefCell<OverlayCache> = RefCell::new(OverlayCache::default());
    static NATIVE_LAYER_SCOPE: RefCell<()> = const { RefCell::new(()) };
}

/// Native merging/destruction may reenter Find. Skip overlays for nested calls on this
/// thread; a blocking mutex would deadlock, and a global try-lock would skip other readers.
pub(super) fn with_native_layer<T>(fallback: T, f: impl FnOnce() -> T) -> T {
    NATIVE_LAYER_SCOPE.with(|scope| {
        let Ok(_borrow) = scope.try_borrow_mut() else {
            return fallback;
        };
        f()
    })
}

fn materialize(key: OverlayKey, name: &str, baseline: *mut FConfigFile) -> Overlay {
    let paths = with_state(|state| state.overrides.paths(name).to_vec());
    let file = (!paths.is_empty()).then(|| unsafe {
        let file = OwnedConfig::copy(baseline);
        for path in &paths {
            let filename = FString::from(path.as_str());
            CALL_ORIGINAL!(ConfigFileAddLayer(file.ptr(), &filename));
        }
        file
    });
    #[cfg(feature = "config_overrides_verbose_hooks")]
    if !paths.is_empty() {
        sdebug!("INI layers for {name}: {}", paths.join(" -> "));
    }
    Overlay { key, file }
}

// Drop this thread's outgoing overlays at the destination boundary. Other threads
// retire their own storage on their next lookup, never while a reader is using it.
pub(super) fn reset_overlays() {
    if !globals_initialized() {
        return;
    }
    with_native_layer((), || {
        let generation = GENERATION.load(Ordering::Acquire);
        let old = CACHE.with(|cache| cache.borrow_mut().retire(generation));
        drop(old);
    });
}

pub(crate) fn route_config(
    cache: *mut FConfigCacheIni,
    filename: &FString,
    baseline: *mut FConfigFile,
) -> *mut FConfigFile {
    if CACHE.with(|cache| cache.borrow().generation) != GENERATION.load(Ordering::Acquire) {
        reset_overlays();
    }
    with_native_layer(baseline, || {
        // Find borrows its caller's const FString for the duration of the call.
        // Compare it in place; only first discovery needs an owned key or UTF-16 decode.
        let cached = CACHE.with(|overlays| overlays.borrow().find(cache, filename.as_slice(), baseline));
        if let Some(file) = cached {
            return file;
        }
        let Ok(name) = filename.copy_to_string() else {
            return baseline;
        };
        if config_filename(&name).is_some_and(|name| name.eq_ignore_ascii_case("Game.ini")) {
            super::remember_game_config(cache, name.clone());
        }
        // Even known cache addresses have no lifetime lease across map boundaries.
        // Materialize once from Find's current baseline, including late native/mod configs.
        // Paths are already selected; only UE's one-time INI read/merge remains.
        let key = OverlayKey {
            cache_address: cache as usize,
            filename: filename.as_slice().into(),
            baseline_address: baseline as usize,
        };
        let entry = materialize(key, &name, baseline);
        let result = entry.file.as_ref().map_or(baseline, OwnedConfig::ptr);
        // Retain older identities until the boundary: native readers may still borrow them.
        CACHE.with(|cache| cache.borrow_mut().files.push(entry));
        result
    })
}

#[cfg(test)]
mod tests {
    use super::super::selection::{expand_layers, select_layers, MapContext};
    use super::*;
    use rstest::{fixture, rstest};

    #[fixture]
    fn overlay_cache() -> OverlayCache {
        OverlayCache {
            generation: 7,
            files: vec![Overlay {
                key: OverlayKey {
                    cache_address: 1,
                    filename: "Custom.ini\0".encode_utf16().collect(),
                    baseline_address: 0x1000,
                },
                file: None,
            }],
        }
    }

    #[rstest]
    #[case::matching(1, "Custom.ini\0", 0x1000, true)]
    #[case::different_cache(2, "Custom.ini\0", 0x1000, false)]
    #[case::different_filename(1, "Other.ini\0", 0x1000, false)]
    #[case::different_baseline(1, "Custom.ini\0", 0x2000, false)]
    fn routing_requires_current_baseline_cache_and_filename(
        overlay_cache: OverlayCache,
        #[case] cache_address: usize,
        #[case] filename: &str,
        #[case] baseline_address: usize,
        #[case] expected_match: bool,
    ) {
        let filename = filename.encode_utf16().collect::<Box<[TCHAR]>>();
        let baseline = std::ptr::without_provenance_mut(baseline_address);
        assert_eq!(
            overlay_cache.find(
                std::ptr::without_provenance_mut(cache_address),
                &filename,
                baseline
            ),
            expected_match.then_some(baseline)
        );
    }

    #[rstest]
    fn cache_retirement_preserves_current_generation_and_retires_old_entries(
        mut overlay_cache: OverlayCache,
    ) {
        let cache = &mut overlay_cache;
        let filename = "Custom.ini\0".encode_utf16().collect::<Box<[TCHAR]>>();
        let cache_pointer = std::ptr::without_provenance_mut::<FConfigCacheIni>(1);
        let baseline = std::ptr::without_provenance_mut(0x1000);
        // Continuing GameMode creation keeps this generation's borrowed pointers.
        assert!(cache.retire(7).is_empty());
        assert_eq!(
            cache.find(cache_pointer, &filename, baseline),
            Some(baseline)
        );
        // Destination selection and failed-load restoration both publish a new generation.
        let retired = cache.retire(8);
        assert_eq!(retired.len(), 1);
        assert_eq!(cache.find(cache_pointer, &filename, baseline), None);
        // Detached entries remain owned until after the cache borrow is released.
        assert_eq!(retired[0].key.baseline_address, baseline as usize);
    }

    #[test]
    fn nested_native_calls_use_baseline_and_release_scope() {
        assert_eq!(
            with_native_layer("baseline", || {
                with_native_layer("nested baseline", || panic!("nested overlay must not run"))
            }),
            "nested baseline"
        );
        assert_eq!(with_native_layer("baseline", || "overlay"), "overlay");
    }

    #[test]
    fn native_scope_is_released_on_unwind() {
        let result = std::panic::catch_unwind(|| {
            with_native_layer((), || panic!("test unwind"));
        });
        assert!(result.is_err());
        assert!(with_native_layer(false, || true));
    }

    #[test]
    fn native_scope_does_not_bypass_other_threads() {
        assert!(with_native_layer(false, || {
            std::thread::spawn(|| with_native_layer(false, || true))
                .join()
                .unwrap()
        }));
    }

    #[test]
    fn destination_preparation_freezes_paths_for_late_config_discovery() {
        let root = std::env::temp_dir().join(format!("unchained-prepared-{}", std::process::id()));
        let root = root.to_str().unwrap();
        for (folder, config) in [
            ("maps/~", "Game.ini"),
            ("maps/A", "Game.ini"),
            ("profiles/busy", "Game.ini"),
            ("indices/2", "Game.ini"),
            ("indices/2", "ModConfig.ini"),
            ("maps/B", "Engine.ini"),
        ] {
            let folder = format!("{root}/ini-overrides/{folder}");
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(format!("{folder}/{config}"), "[Test]\nValue=1").unwrap();
        }
        let prepare = |map, index, profile| {
            ConfigLayers::new(
                root,
                &expand_layers(root, &select_layers(&MapContext::new(map, index), profile)),
            )
        };
        let outgoing = prepare("A", Some(2), Some("busy"));
        // Production paths use forward slashes even when the host temp root does not.
        let expected_root = root.replace('\\', "/");
        assert_eq!(
            outgoing.paths(r"C:\Saved\gAmE.INI"),
            [
                format!("{expected_root}/ini-overrides/maps/~/Game.ini"),
                format!("{expected_root}/ini-overrides/maps/A/Game.ini"),
                format!("{expected_root}/ini-overrides/profiles/busy/Game.ini"),
                format!("{expected_root}/ini-overrides/indices/2/Game.ini"),
            ]
        );
        // Exercise Windows-style roots on every host, not only Windows CI.
        let windows_root = root.replace('/', "\\");
        let windows_layers = ConfigLayers::new(
            &windows_root,
            &expand_layers(
                root,
                &select_layers(&MapContext::new("A", Some(2)), Some("busy")),
            ),
        );
        assert_eq!(windows_layers.paths("Game.ini"), outgoing.paths("Game.ini"));
        let late_path = format!("{root}/ini-overrides/indices/2/ModConfig.ini");
        std::fs::remove_file(&late_path).unwrap();
        // A first read by a mod uses the frozen selection, not another directory scan.
        assert_eq!(
            outgoing.paths("ModConfig.ini"),
            [late_path.replace('\\', "/")]
        );
        assert!(outgoing.paths("Unknown.ini").is_empty());
        assert!(outgoing.paths("../CON.ini").is_empty());

        let destination = prepare("B", None, None);
        let mut refreshed = outgoing
            .categories()
            .collect::<std::collections::HashSet<_>>();
        refreshed.extend(destination.categories());
        assert_eq!(
            refreshed,
            ["game", "modconfig", "engine"].map(String::from).into()
        );
        assert_eq!(destination.paths("Game.ini").len(), 1);
        assert!(destination.paths("ModConfig.ini").is_empty());
        // Failed-load restoration selects A again, without retaining B's paths.
        let restored = prepare("A", Some(2), Some("busy"));
        assert_eq!(restored.paths("Game.ini"), outgoing.paths("Game.ini"));
        assert!(restored.paths("Engine.ini").is_empty());
        assert!(restored.paths("ModConfig.ini").is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
