//! Destination selection → ordered layer paths → private native config copies.
//!
//! `travel` owns startup, accepted rotation, seamless continuity, and rollback.
//! `selection` validates and orders map/profile/index folders.
//! `layering` prepares paths and lazily merges baseline copies on native Find.
//! This module coordinates those steps and refreshes existing class defaults.
mod layering;
mod selection;
mod settings;
mod travel;

pub use settings::{parse_server_settings, ServerSettings};

use std::sync::{
    atomic::{AtomicU64, Ordering},
    OnceLock,
};
use std::{collections::HashSet, sync::Mutex};

use crate::{
    resolvers::ini::{o_ConfigCacheFind, o_ConfigCacheSetInt, o_ConfigFileGetInt, o_LoadConfig},
    tools::hook_globals::{cli_args, globals, globals_initialized},
    ue::{EObjectFlags, FConfigCacheIni, FString, UObject},
};

pub(crate) use layering::route_config;
use layering::{reset_overlays, with_native_layer, ConfigLayers};
use selection::{expand_layers, select_layers, MapContext};
use travel::{Destination, TravelState};

#[derive(Default)]
struct State {
    travel: TravelState,
    layers: ConfigLayers,
    pending_refresh: HashSet<String>,
    refreshing_generation: Option<u64>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);
static GENERATION: AtomicU64 = AtomicU64::new(0);

fn with_state<T>(f: impl FnOnce(&mut State) -> T) -> T {
    let mut state = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(state.get_or_insert_with(State::default))
}

fn profile_root() -> Option<&'static str> {
    crate::tools::hook_globals::CLI_ARGS
        .get()?
        .unchained_profile_location
        .as_deref()
}

impl State {
    // Called only at destination boundaries, including failed-load restoration.
    fn prepare_layers(&mut self, root: &str) {
        let context = &self.travel.current.selection;
        let profile = context
            .index
            .and_then(|index| server_settings(root).profile_at(index));
        let selected = select_layers(context, profile);
        let expanded = expand_layers(root, &selected);
        let layers = ConfigLayers::new(root, &expanded);
        // Removed categories must also reload their baseline defaults.
        self.pending_refresh.extend(self.layers.categories());
        self.pending_refresh.extend(layers.categories());
        self.layers = layers;
        GENERATION.store(self.travel.current.generation, Ordering::Release);
    }

    fn enter_map(&mut self, root: &str, map: &str, world: Option<usize>) -> Destination {
        let previous = self.travel.enter(map, world);
        if previous.generation != self.travel.current.generation {
            self.prepare_layers(root);
        }
        previous
    }
}

// Keep outgoing categories pending until a ready refresh completes. Native calls may
// reenter selection; neither repeat an in-flight generation nor clear a newer one.
fn refresh_selected(refresh: impl FnOnce(&HashSet<String>) -> bool) {
    let Some(pending) = with_state(|state| {
        let generation = state.travel.current.generation;
        if state.pending_refresh.is_empty() || state.refreshing_generation == Some(generation) {
            return None;
        }
        state.refreshing_generation = Some(generation);
        Some(DefaultRefresh {
            generation,
            categories: state.pending_refresh.clone(),
        })
    }) else {
        return;
    };
    let refreshed = refresh(&pending.categories);
    with_state(|state| {
        if state.refreshing_generation == Some(pending.generation) {
            state.refreshing_generation = None;
        }
        if refreshed && state.travel.current.generation == pending.generation {
            state.pending_refresh.clear();
        }
    });
}

struct DefaultRefresh {
    generation: u64,
    categories: HashSet<String>,
}

// Snapshot paths under the state lock; native merging must run after it is released.
fn layer_paths(filename: &str) -> Vec<String> {
    with_state(|state| state.layers.paths(filename).to_vec())
}

fn server_settings(root: &str) -> &'static ServerSettings {
    static SETTINGS: OnceLock<ServerSettings> = OnceLock::new();
    SETTINGS.get_or_init(|| {
        let path = format!("{}/unchained.yaml", root.trim_end_matches(['/', '\\']));
        match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|yaml| parse_server_settings(&yaml).map_err(|e| e.to_string()))
        {
            Ok(settings) => settings,
            Err(error) => {
                if std::path::Path::new(&path).exists() {
                    swarn!("Cannot load map profiles from {path}: {error}");
                }
                ServerSettings::default()
            }
        }
    })
}

// Keep the native cache identity and filename, not a borrowed FConfigFile pointer.
#[derive(Clone)]
struct ConfigLocation {
    cache_address: usize,
    filename: String,
}

static GAME_CONFIG: Mutex<Option<ConfigLocation>> = Mutex::new(None);

fn game_config() -> Option<ConfigLocation> {
    GAME_CONFIG
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

fn remember_game_config(cache: *mut FConfigCacheIni, filename: String) {
    *GAME_CONFIG.lock().unwrap_or_else(|e| e.into_inner()) = Some(ConfigLocation {
        cache_address: cache as usize,
        filename,
    });
}

const ROTATION_SECTION: &widestring::U16CStr = widestring::u16cstr!("/Script/TBL.TBLGameMode");
const ROTATION_INDEX: &widestring::U16CStr = widestring::u16cstr!("MapListIndex");

fn capture_startup(map: &str) {
    let starting_map = cli_args().next_map.as_deref();
    if !with_state(|state| state.travel.can_capture_startup(map, starting_map)) {
        return;
    }
    let Some(config) = game_config() else {
        return;
    };
    let filename = FString::from(config.filename.as_str());
    let baseline = CALL_ORIGINAL!(ConfigCacheFind(
        config.cache_address as *mut FConfigCacheIni,
        &filename,
        false
    ));
    if baseline.is_null() {
        return;
    }
    let mut index = 0;
    let index = CALL_ORIGINAL!(ConfigFileGetInt(
        baseline,
        ROTATION_SECTION.as_ptr(),
        ROTATION_INDEX.as_ptr(),
        &mut index
    ))
    .then_some(index);
    with_state(|state| {
        state.travel.capture_startup(map, starting_map, index);
    });
}

fn preserve_rotation_index(index: i32) {
    let Some(config) = game_config() else {
        return;
    };
    let filename = FString::from(config.filename.as_str());
    // Rotation SaveConfig has already written to the non-saving outgoing overlay.
    // Bypass Find layering for this one scalar so the next generation starts at the live slot.
    with_native_layer((), || {
        CALL_ORIGINAL!(ConfigCacheSetInt(
            config.cache_address as *mut FConfigCacheIni,
            ROTATION_SECTION.as_ptr(),
            ROTATION_INDEX.as_ptr(),
            index,
            &filename
        ))
    });
}

fn refresh_defaults(categories: &HashSet<String>) -> bool {
    if !globals_initialized() {
        return false;
    }
    // Release the object-array lock before invoking engine code, which can allocate UObjects.
    let candidates = {
        let objects = globals().guobject_array();
        let objects = objects.objects();
        (0..objects.num_elements)
            .filter_map(|index| {
                // Keep the slot's raw provenance for native writes to derived config properties.
                let object = objects.item(index).object;
                let base = unsafe { object.as_ref()? };
                (base
                    .object_flags
                    .contains(EObjectFlags::RF_ClassDefaultObject)
                    && !base.object_flags.intersects(
                        EObjectFlags::RF_BeginDestroyed | EObjectFlags::RF_FinishDestroyed,
                    ))
                .then_some(object as *mut UObject)
            })
            .collect::<Vec<_>>()
    };
    for object in candidates {
        unsafe {
            let base = &(*object).uobject_base_utility.uobject_base;
            if !globals().guobject_array_unchecked().is_live(base) {
                continue;
            }
            let Some(class) = base.class_private.as_ref() else {
                continue;
            };
            if class.class_flags & 0x4 == 0 {
                continue;
            }
            if categories.contains(&class.class_config_name.to_string().to_ascii_lowercase()) {
                CALL_ORIGINAL!(LoadConfig(
                    object,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    0,
                    std::ptr::null_mut()
                ));
            }
        }
    }
    true
}

/// Select destination layers before native map/game-mode initialization.
pub(crate) fn select_map(map: &str, world: Option<usize>) -> Option<Destination> {
    capture_startup(map);
    let root = profile_root()?;
    let previous = with_state(|state| state.enter_map(root, map, world));
    reset_overlays();
    refresh_selected(refresh_defaults);
    Some(previous)
}

pub(crate) fn restore_map(previous: Destination) {
    with_state(|state| {
        state.travel.restore(previous);
        state.prepare_layers(profile_root().unwrap());
    });
    reset_overlays();
    refresh_selected(refresh_defaults);
}

pub(crate) fn capture_rotation(map: &str, index: i32, entry: Option<&str>) -> Option<MapContext> {
    travel::capture_rotation(map, index, entry, preserve_rotation_index)
}

pub(crate) fn accept_travel(pending: Option<MapContext>) {
    with_state(|state| state.travel.accept_rotation(pending));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destination_refresh_retries_deduplicates_and_preserves_pending_categories() {
        let root = std::env::temp_dir().join(format!("unchained-refresh-{}", std::process::id()));
        for (map, category) in [("A", "Engine"), ("B", "Game")] {
            let folder = root.join("ini-overrides/maps").join(map);
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(folder.join(format!("{category}.ini")), "[Test]\nValue=1").unwrap();
        }
        let root = root.to_str().unwrap();
        with_state(|state| *state = State::default());
        let enter = |map, world| with_state(|state| state.enter_map(root, map, world));
        let refreshed = |expected: &[&str]| {
            let mut called = false;
            refresh_selected(|categories| {
                called = true;
                assert_eq!(
                    categories,
                    &expected.iter().map(|s| s.to_string()).collect()
                );
                true
            });
            assert!(called);
        };
        let unchanged = || refresh_selected(|_| panic!("already refreshed"));

        enter("A", None);
        refresh_selected(|_| false); // Globals are not ready: retain the work.
        enter("A", Some(42));
        refreshed(&["engine"]);
        enter("A", Some(42));
        unchanged();
        // A ready ordinary load and its GameMode share a single refresh too.
        enter("A", None);
        refreshed(&["engine"]);
        enter("A", Some(43));
        unchanged();
        enter("A", Some(43));
        unchanged();
        enter("A", Some(44));
        refreshed(&["engine"]);

        let previous = enter("B", None);
        refreshed(&["engine", "game"]); // Removed Engine overrides reload baseline.
        with_state(|state| {
            state.travel.restore(previous);
            state.prepare_layers(root);
        });
        refreshed(&["engine", "game"]);
        unchanged();

        enter("B", None);
        refresh_selected(|_| false);
        enter("C", None);
        refresh_selected(|_| false);
        enter("C", Some(45));
        refreshed(&["engine", "game"]); // Neither skipped generation loses its categories.

        with_state(|state| {
            state
                .travel
                .accept_rotation(Some(MapContext::new("A", Some(2))));
        });
        enter("B", Some(46)); // Seamless transition must keep the final slot pending.
        refreshed(&["game"]);
        enter("A", Some(47));
        assert_eq!(
            with_state(|state| state.travel.current.selection.index),
            Some(2)
        );
        refreshed(&["engine", "game"]);

        enter("B", None);
        refresh_selected(|_| {
            enter("B", Some(48));
            unchanged(); // Native reentry must not recurse into the same refresh.
            enter("C", None); // Publish a newer generation while native code runs.
            true
        });
        refreshed(&["engine", "game"]); // Old completion cannot consume newer work.
        unchanged();

        enter("A", None);
        refresh_selected(|_| {
            enter("B", None);
            refreshed(&["engine", "game"]);
            true
        });
        unchanged(); // Newer nested completion remains authoritative.
    }
}
