//! Conditional runtime layers. UE performs all INI parsing and array merging.
pub(crate) mod native;
mod selection;
mod settings;
mod travel;

pub use settings::{parse_server_settings, ServerSettings};

use std::sync::OnceLock;
use std::{collections::HashSet, sync::Mutex};

pub use selection::{build_override_paths, build_profile_override_path, OverrideLayer};
use selection::{config_filename, expand_layers, select_layers, MapContext};
use travel::{enter_context, Context, StartupSelection};

#[derive(Default)]
struct State {
    context: Context,
    pending_rotation: Option<MapContext>,
    startup: StartupSelection,
    // Categories touched by the preceding map must also be refreshed on removal.
    categories: HashSet<String>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

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

fn layers(context: &MapContext, root: &str) -> Vec<(OverrideLayer, String)> {
    let profile = context
        .index
        .and_then(|index| server_settings(root).profile_at(index));
    expand_layers(root, &select_layers(context, profile))
}

fn override_paths(context: &MapContext, filename: &str) -> Vec<String> {
    let Some(root) = profile_root() else {
        return Vec::new();
    };
    let Some(config) = config_filename(filename) else {
        return Vec::new();
    };
    build_override_paths(root, &[config], &layers(context, root))
        .into_iter()
        .filter(|path| std::path::Path::new(path).is_file())
        .collect()
}

fn categories_for(context: &MapContext) -> HashSet<String> {
    let Some(root) = profile_root() else {
        return HashSet::new();
    };
    let mut categories = HashSet::new();
    for (layer, slot) in layers(context, root) {
        let folder = build_profile_override_path(root, layer, &slot, "");
        let Ok(entries) = std::fs::read_dir(folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(name) = config_filename(&name) {
                categories.insert(name[..name.len() - 4].to_ascii_lowercase());
            }
        }
    }
    categories
}

/// Called with the destination URL, never the outgoing world's map or rotation index.
fn enter_map(map: &str, world: Option<usize>) -> Option<(Context, HashSet<String>)> {
    profile_root()?;
    let (previous, context) = with_state(|state| {
        let previous = enter_context(&mut state.context, &mut state.pending_rotation, map, world);
        (previous, state.context.clone())
    });
    let new_categories = categories_for(&context.selection);
    let mut categories = new_categories.clone();
    with_state(|state| {
        categories.extend(state.categories.iter().cloned());
        state.categories = new_categories;
    });
    Some((previous, categories))
}

fn server_settings(root: &str) -> &'static ServerSettings {
    static SETTINGS: OnceLock<ServerSettings> = OnceLock::new();
    SETTINGS.get_or_init(|| {
        let path = format!(
            "{}/ini-overrides/unchained.yaml",
            root.trim_end_matches(['/', '\\'])
        );
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
        let yaml = std::fs::read_to_string(format!("{root}/ini-overrides/unchained.yaml")).unwrap();
        let settings = super::parse_server_settings(&yaml).unwrap();
        let profile = settings.profile_at(1).unwrap();
        assert!(super::selection::valid_component(profile));
        assert!(std::path::Path::new(&super::build_profile_override_path(
            root,
            super::OverrideLayer::MapProfile,
            profile,
            "Engine.ini"
        ))
        .is_file());
    }
}
