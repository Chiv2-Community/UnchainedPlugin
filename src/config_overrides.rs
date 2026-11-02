//! Conditional runtime layers. UE performs all INI parsing and array merging.
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
use layering::ConfigLayers;
use layering::{reset_overlays, with_native_layer};
use selection::{expand_layers, select_layers, LayerSelection, MapContext};
use travel::{enter_context, restore_context, Context, StartupSelection};

#[derive(Default)]
struct State {
    context: Context,
    pending_rotation: Option<MapContext>,
    startup: StartupSelection,
    overrides: ConfigLayers,
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

fn layers(context: &MapContext, root: &str) -> Vec<LayerSelection> {
    let profile = context
        .index
        .and_then(|index| server_settings(root).profile_at(index));
    expand_layers(root, &select_layers(context, profile))
}

// Called only at destination boundaries, including failed-load restoration.
fn prepare_context(state: &mut State, root: &str) -> HashSet<String> {
    let overrides = ConfigLayers::new(root, &layers(&state.context.selection, root));
    // Removed categories must also reload their baseline defaults.
    let mut categories = state.overrides.categories().collect::<HashSet<_>>();
    categories.extend(overrides.categories());
    state.overrides = overrides;
    GENERATION.store(state.context.generation, Ordering::Release);
    categories
}

/// Called with the destination URL, never the outgoing world's map or rotation index.
fn enter_map(map: &str, world: Option<usize>) -> Option<MapChange> {
    let root = profile_root()?;
    Some(with_state(|state| {
        let previous = enter_context(&mut state.context, &mut state.pending_rotation, map, world);
        let categories = if previous.generation != state.context.generation {
            prepare_context(state, root)
        } else {
            state.overrides.categories().collect()
        };
        MapChange {
            previous,
            categories,
        }
    }))
}

struct MapChange {
    previous: Context,
    categories: HashSet<String>,
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

struct ConfigKey {
    section: Vec<u16>,
    property: Vec<u16>,
}

fn index_key() -> ConfigKey {
    ConfigKey {
        section: "/Script/TBL.TBLGameMode\0".encode_utf16().collect(),
        property: "MapListIndex\0".encode_utf16().collect(),
    }
}

fn capture_startup(map: &str) {
    let starting_map = cli_args().next_map.as_deref();
    if !with_state(|state| state.startup.can_capture(map, starting_map)) {
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
    let key = index_key();
    let mut index = 0;
    let index = CALL_ORIGINAL!(ConfigFileGetInt(
        baseline,
        key.section.as_ptr(),
        key.property.as_ptr(),
        &mut index
    ))
    .then_some(index);
    with_state(|state| {
        if let Some(selection) = state.startup.capture(map, starting_map, index) {
            state.pending_rotation = Some(selection);
        }
    });
}

fn preserve_rotation_index(index: i32) {
    let Some(config) = game_config() else {
        return;
    };
    let filename = FString::from(config.filename.as_str());
    let key = index_key();
    // Rotation SaveConfig has already written to the non-saving outgoing overlay.
    // Bypass Find layering for this one scalar so the next generation starts at the live slot.
    with_native_layer((), || {
        CALL_ORIGINAL!(ConfigCacheSetInt(
            config.cache_address as *mut FConfigCacheIni,
            key.section.as_ptr(),
            key.property.as_ptr(),
            index,
            &filename
        ))
    });
}

fn refresh_defaults(categories: &HashSet<String>) {
    if categories.is_empty() || !globals_initialized() {
        return;
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
}

pub(crate) fn select_map(map: &str, world: Option<usize>) -> Option<Context> {
    capture_startup(map);
    let change = enter_map(map, world)?;
    reset_overlays();
    refresh_defaults(&change.categories);
    Some(change.previous)
}

pub(crate) fn restore_map(previous: Context) {
    let categories = with_state(|state| {
        restore_context(&mut state.context, &mut state.pending_rotation, previous);
        prepare_context(state, profile_root().unwrap())
    });
    reset_overlays();
    refresh_defaults(&categories);
}

pub(crate) fn capture_rotation(map: &str, index: i32, entry: Option<&str>) -> Option<MapContext> {
    travel::capture_rotation(map, index, entry, preserve_rotation_index)
}

pub(crate) fn accept_travel(pending: Option<MapContext>) {
    // Every accepted travel replaces the previous snapshot, including manual/unmatched travel.
    with_state(|state| {
        state.startup.finish();
        state.pending_rotation = pending;
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn profiles_parse_and_fixture_exists() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/test-resources/test-configs");
        let yaml = std::fs::read_to_string(format!("{root}/unchained.yaml")).unwrap();
        let settings = super::parse_server_settings(&yaml).unwrap();
        let profile = settings.profile_at(1).unwrap();
        assert!(super::selection::valid_component(profile));
        assert!(
            std::path::Path::new(&super::selection::build_profile_override_path(
                root,
                super::selection::OverrideLayer::MapProfile,
                profile,
                "Engine.ini"
            ))
            .is_file()
        );
    }
}
