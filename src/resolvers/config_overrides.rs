//! Conditional runtime layers. UE performs all INI parsing and array merging.
pub(crate) mod native;
mod prepared;
mod selection;
mod settings;
mod travel;

pub use settings::{parse_server_settings, ServerSettings};

use std::sync::{
    atomic::{AtomicU64, Ordering},
    OnceLock,
};
use std::{collections::HashSet, sync::Mutex};

use prepared::PreparedOverrides;
use selection::{expand_layers, select_layers, LayerSelection, MapContext};
use travel::{enter_context, Context, StartupSelection};

#[derive(Default)]
struct State {
    context: Context,
    pending_rotation: Option<MapContext>,
    startup: StartupSelection,
    overrides: PreparedOverrides,
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
    let overrides = PreparedOverrides::new(root, &layers(&state.context.selection, root));
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
