use std::cell::Cell;

use crate::{
    game::{
        chivalry2::ATBLGameMode,
        engine::UWorld,
    },
    tools::hook_globals::globals,
};

// Lowercased, normalized form of /Game/Mods/AgMods/ for case-insensitive matching.
const MOD_ROOT_LOWER: &str = "/game/mods/agmods/";
const INI_OVERRIDES_DIR: &str = "ini-overrides";
const MAPS_DIR: &str = "maps";
const MAP_INDICES_DIR: &str = "map-indices";
#[cfg(feature = "map_profiles")]
const MAP_PROFILES_DIR: &str = "map-profiles";
#[cfg(feature = "map_profiles")]
const SERVER_SETTINGS_FILE: &str = "unchained.yaml";

/// One override layer below `<profile>/ini-overrides/`. Layers are applied in the order they are
/// collected, so a later layer takes precedence over an earlier one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverrideLayer {
    /// `maps/<MapName>/` keyed by the current map's leaf name.
    Map,
    /// `map-indices/<index>/` keyed by the map rotation index.
    #[cfg_attr(feature = "map_profiles", allow(dead_code))]
    MapIndex,
    /// `map-profiles/<profile>/` keyed by the profile name mapped to the rotation index.
    #[cfg(feature = "map_profiles")]
    MapProfile,
}

impl OverrideLayer {
    pub fn dir(self) -> &'static str {
        match self {
            OverrideLayer::Map => MAPS_DIR,
            OverrideLayer::MapIndex => MAP_INDICES_DIR,
            #[cfg(feature = "map_profiles")]
            OverrideLayer::MapProfile => MAP_PROFILES_DIR,
        }
    }
}

#[allow(unused_macros)]
#[cfg(feature = "config_overrides_verbose_hooks")]
macro_rules! config_debug {
    ($($arg:tt)*) => {
        crate::sdebug!($($arg)*);
    };
}

#[allow(unused_macros)]
#[cfg(not(feature = "config_overrides_verbose_hooks"))]
macro_rules! config_debug {
    ($($arg:tt)*) => {};
}

thread_local! {
    // Prevent recursive LoadConfig calls from re-entering the same override chain.
    // The game loads config files in nested ways, and we only want the first pass to
    // trigger profile-based follow-up loads, not an infinite loop.
    pub static LOAD_CONFIG_REENTRY: Cell<bool> = const { Cell::new(false) };
}

fn is_under_mod_root(lowered_path: &str) -> bool {
    // Everything below /Game/Mods/AgMods/ (any subfolder depth) is mod-owned. The trailing slash
    // keeps sibling folders like /Game/Mods/AgModsOther from matching.
    lowered_path.contains(MOD_ROOT_LOWER)
}

fn is_mod_config_path(original_name: &str) -> bool {
    // Unreal config files like Engine.ini can be requested for both base game assets and mod assets.
    // We only want the override layer to activate for the mod case, so we check whether the file
    // path actually lives under one of the known mod roots.
    let normalized = original_name.replace('\\', "/");
    let lowered = normalized.to_ascii_lowercase();

    let is_ue_config = ["engine.ini", "game.ini", "defaultengine.ini"]
        .iter()
        .any(|suffix| lowered.ends_with(suffix));

    is_ue_config && is_under_mod_root(&lowered)
}

pub fn object_path_is_mod_root(object_path: &str) -> bool {
    // Some LoadConfig calls do not carry a useful filename, but the object path still tells us whether
    // the asset belongs to a mod hierarchy. That is enough to decide whether the override loader is
    // relevant even when the filename itself is generic or empty.
    let lowered = object_path.replace('\\', "/").to_ascii_lowercase();
    is_under_mod_root(&lowered)
}

pub fn build_profile_override_path(profile_root: &str, layer: OverrideLayer, slot: &str, config_name: &str) -> String {
    // This function builds the override path under the unchained profile directory. The directory
    // layout is: <profile>/ini-overrides/<maps|map-indices|map-profiles>/<slot>/<Config>.ini.
    // We normalize both separators and trimming so the generated path matches what the engine expects.
    let clean_root = profile_root.trim_end_matches(['/', '\\']);
    let layer_dir = layer.dir();
    let clean_slot = slot.trim_matches(['/', '\\']);
    let clean_name = config_name.trim_matches(['/', '\\']);
    format!("{clean_root}/{INI_OVERRIDES_DIR}/{layer_dir}/{clean_slot}/{clean_name}").replace('\\', "/")
}

pub fn build_override_paths(profile_root: &str, candidates: &[String], layers: &[(OverrideLayer, String)]) -> Vec<String> {
    // Iterate layer-major so every path of an earlier layer is loaded before any path of a later
    // layer, regardless of how many config candidates there are. The base config is not part of
    // this list; resolve_profile_override_paths_for_config prepends it.
    let mut paths = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for (layer, slot) in layers {
        for config_name in candidates {
            let path = build_profile_override_path(profile_root, *layer, slot, config_name);
            if seen.insert(path.clone()) {
                paths.push(path);
            }
        }
    }

    paths
}

pub fn current_map_rotation_folder_name() -> Option<String> {
    // When the world is already loaded, the active map name is a strong discriminator for overrides.
    // We strip the full asset path down to the map's leaf name so that the override directory can be
    // keyed by a readable map identifier like TDM_QuarryKP.
    if let Some(world) = globals().world() {
        let map_name = crate::resolvers::unchained_integration::get_current_map_name(world)?;
        let last = map_name.rsplit('/').next().unwrap_or(&map_name);
        let cleaned = last.trim_matches(['/', '\\']);
        if !cleaned.is_empty() {
            return Some(cleaned.to_string());
        }
    }
    None
}

fn current_map_list_index() -> Option<i32> {
    // This index is the rotation slot currently active in the match.
    let world = globals().world()?;
    let world_ref = unsafe { &*(world as *mut UWorld) };
    let game_mode = unsafe { (world_ref.authority_game_mode as *mut ATBLGameMode).as_ref() }?;
    let index = game_mode.map_list_index;
    (index >= 0).then_some(index)
}

#[cfg_attr(feature = "map_profiles", allow(dead_code))]
pub fn current_map_rotation_index() -> Option<String> {
    // The match rotation index is the second override key. It is useful when we want to target a slot
    // rather than a specific map name, especially during rotation or when map names are not stable.
    current_map_list_index().map(|index| index.to_string())
}

#[cfg(feature = "map_profiles")]
#[derive(Debug, Default, serde::Deserialize)]
pub struct ServerSettings {
    /// Rotation index -> map-profiles subfolder name.
    #[serde(default)]
    pub map_profiles: std::collections::HashMap<i32, String>,
}

#[cfg(feature = "map_profiles")]
static SERVER_SETTINGS: std::sync::OnceLock<ServerSettings> = std::sync::OnceLock::new();

#[cfg(feature = "map_profiles")]
pub fn parse_server_settings(yaml: &str) -> Result<ServerSettings, serde_yaml::Error> {
    // An empty file deserializes to null, which should mean "no settings" rather than an error.
    if yaml.trim().is_empty() {
        return Ok(ServerSettings::default());
    }
    serde_yaml::from_str(yaml)
}

#[cfg(feature = "map_profiles")]
fn server_settings(profile_root: &str) -> &'static ServerSettings {
    // Loaded once on first use. A missing or invalid file disables map-profiles instead of failing.
    SERVER_SETTINGS.get_or_init(|| {
        let clean_root = profile_root.trim_end_matches(['/', '\\']);
        let path = format!("{clean_root}/{INI_OVERRIDES_DIR}/{SERVER_SETTINGS_FILE}");
        let settings = std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|yaml| parse_server_settings(&yaml).map_err(|e| e.to_string()));
        match settings {
            Ok(settings) => {
                #[cfg(feature = "config_overrides_verbose_hooks")]
                crate::sinfo!("[config-overrides] loaded {} map profile(s) from {}", settings.map_profiles.len(), path);
                settings
            }
            Err(_e) => {
                #[cfg(feature = "config_overrides_verbose_hooks")]
                crate::swarn!("⚠️ [config-overrides] could not load {}: {}", path, _e);
                ServerSettings::default()
            }
        }
    })
}

#[cfg(feature = "map_profiles")]
pub fn current_map_profile_name(profile_root: &str) -> Option<String> {
    // Replaces the map-indices layer: the rotation index is looked up in unchained.yaml and the
    // resulting name selects the map-profiles/<name>/ folder. Unmapped indices get no profile layer.
    let index = current_map_list_index()?;
    let profile = server_settings(profile_root).map_profiles.get(&index)?;
    let cleaned = profile.trim().trim_matches(['/', '\\']);
    (!cleaned.is_empty()).then(|| cleaned.to_string())
}

pub fn resolve_profile_override_paths_for_config(original_name: Option<&str>, object_path: Option<&str>) -> Vec<String> {
    // This is the main override resolver: take the current config load request, decide whether it is
    // a mod-owned config, and then return the profile override files to load afterwards. The pattern is
    // deliberate: first the normal engine load, then profile overrides on top. That preserves UE's
    // default precedence while allowing a second layer to add server-specific values.
    let original_path = original_name.filter(|value| !value.trim().is_empty());
    let object_path = object_path.filter(|value| !value.trim().is_empty());

    let has_mod_context = original_path
        .is_some_and(|name| is_mod_config_path(name))
        || object_path.is_some_and(|path| object_path_is_mod_root(path));

    // Non-mod assets are the common case; skip them silently.
    if !has_mod_context {
        return vec![];
    }

    let profile_root = match crate::tools::hook_globals::CLI_ARGS.get().and_then(|cli| cli.unchained_profile_location.as_deref()) {
        Some(root) => root,
        None => {
            #[cfg(feature = "config_overrides_verbose_hooks")]
            crate::swarn!("⚠️ [config-overrides] missing unchained_profile_location, cannot resolve overrides");
            return vec![];
        }
    };

    let mut candidates = Vec::new();

    if let Some(name) = original_path {
        let name = name.replace('\\', "/");
        let file_name = name.rsplit('/').next().unwrap_or(&name).trim();
        if !file_name.is_empty() {
            candidates.push(file_name.to_string());
        }
    }

    if candidates.is_empty() {
        for fallback in ["Engine.ini", "Game.ini", "DefaultEngine.ini"] {
            candidates.push(fallback.to_string());
        }
    }

    // The object path is only used for the mod-context check above, never as a config filename:
    // its leaf (e.g. GiantSlayers.Default__GiantSlayers_C) is an object name, not an ini.

    // Precedence (last wins): base -> maps -> map-indices | map-profiles.
    let mut layers = Vec::new();
    if let Some(map_name) = current_map_rotation_folder_name() {
        layers.push((OverrideLayer::Map, map_name));
    }
    #[cfg(not(feature = "map_profiles"))]
    if let Some(map_index) = current_map_rotation_index() {
        layers.push((OverrideLayer::MapIndex, map_index));
    }
    #[cfg(feature = "map_profiles")]
    if let Some(profile) = current_map_profile_name(profile_root) {
        layers.push((OverrideLayer::MapProfile, profile));
    }

    let override_paths = build_override_paths(profile_root, &candidates, &layers);

    #[cfg(feature = "config_overrides_verbose_hooks")]
    log_override_summary(profile_root, original_path, object_path, &layers, &override_paths);

    let mut paths = Vec::new();
    // Always load the base ini (resolved via GetConfigName, e.g. Saved_Unchained/.../Engine.ini) explicitly first.
    // The hook's initial LoadConfig call often has a null filename and does not resolve to the same file.
    paths.push(original_path.map(|name| name.replace('\\', "/")).unwrap_or_default());
    paths.extend(override_paths);
    paths
}

#[cfg(feature = "config_overrides_verbose_hooks")]
fn log_override_summary(
    profile_root: &str,
    original_path: Option<&str>,
    object_path: Option<&str>,
    layers: &[(OverrideLayer, String)],
    override_paths: &[String],
) {
    // One line per mod object, and only when something is worth seeing: an override file that
    // actually exists, or a problem that prevents overrides from being resolved. Mod objects
    // without any override files on disk stay silent.
    let object = object_path.unwrap_or("<unknown object>");

    if original_path.is_none() {
        #[cfg(feature = "config_overrides_verbose_hooks")]
        crate::swarn!("⚠️ [config-overrides] {}: no base config name (filename and GetConfigName empty)", object);
    }
    if layers.is_empty() {
        #[cfg(feature = "config_overrides_verbose_hooks")]
        crate::swarn!("⚠️ [config-overrides] {}: no map name or rotation slot available, no overrides resolved", object);
        return;
    }

    let overrides_root = format!("{}/{INI_OVERRIDES_DIR}/", profile_root.trim_end_matches(['/', '\\']).replace('\\', "/"));
    let existing = override_paths
        .iter()
        .filter(|path| std::path::Path::new(path.as_str()).is_file())
        .map(|path| path.strip_prefix(overrides_root.as_str()).unwrap_or(path))
        .collect::<Vec<_>>();

    if !existing.is_empty() {
        let slots = layers.iter().map(|(layer, slot)| format!("{}={slot}", layer.dir())).collect::<Vec<_>>();
        
        #[cfg(feature = "config_overrides_verbose_hooks")]
        config_debug!("🔀 [config-overrides] {} [{}] -> {}", object, slots.join(", "), existing.join(", "));
    }
}

#[cfg(test)]
mod tests {
    use super::{build_override_paths, build_profile_override_path, OverrideLayer};

    #[test]
    fn builds_per_map_override_path_for_engine_ini() {
        let path = build_profile_override_path(r"C:\Profiles\Server", OverrideLayer::Map, "TDM_QuarryKP", "Engine.ini");
        assert_eq!(path, "C:/Profiles/Server/ini-overrides/maps/TDM_QuarryKP/Engine.ini");
    }

    #[test]
    fn builds_map_index_override_path_for_game_ini() {
        let path = build_profile_override_path(r"C:/Profiles/Server/", OverrideLayer::MapIndex, "2", "Game.ini");
        assert_eq!(path, "C:/Profiles/Server/ini-overrides/map-indices/2/Game.ini");
    }

    #[test]
    fn override_paths_are_layer_major_and_exclude_base() {
        let candidates = vec!["Engine.ini".to_string(), "Game.ini".to_string()];
        let layers = vec![
            (OverrideLayer::Map, "TDM_QuarryKP".to_string()),
            (OverrideLayer::MapIndex, "1".to_string()),
        ];
        let paths = build_override_paths("C:/P", &candidates, &layers);
        assert_eq!(paths, vec![
            "C:/P/ini-overrides/maps/TDM_QuarryKP/Engine.ini",
            "C:/P/ini-overrides/maps/TDM_QuarryKP/Game.ini",
            "C:/P/ini-overrides/map-indices/1/Engine.ini",
            "C:/P/ini-overrides/map-indices/1/Game.ini",
        ]);
    }

    #[test]
    fn duplicate_map_path_does_not_skip_index_layer() {
        let candidates = vec!["Engine.ini".to_string(), "engine.ini".to_string(), "Engine.ini".to_string()];
        let layers = vec![
            (OverrideLayer::Map, "TDM_QuarryKP".to_string()),
            (OverrideLayer::MapIndex, "1".to_string()),
        ];
        let paths = build_override_paths("C:/P", &candidates, &layers);
        assert_eq!(paths, vec![
            "C:/P/ini-overrides/maps/TDM_QuarryKP/Engine.ini",
            "C:/P/ini-overrides/maps/TDM_QuarryKP/engine.ini",
            "C:/P/ini-overrides/map-indices/1/Engine.ini",
            "C:/P/ini-overrides/map-indices/1/engine.ini",
        ]);
    }

    #[cfg(feature = "map_profiles")]
    #[test]
    fn builds_map_profile_override_path() {
        let path = build_profile_override_path("C:/P", OverrideLayer::MapProfile, "night_tdm", "Engine.ini");
        assert_eq!(path, "C:/P/ini-overrides/map-profiles/night_tdm/Engine.ini");
    }

    #[cfg(feature = "map_profiles")]
    #[test]
    fn parses_server_settings_map_profiles() {
        let settings = super::parse_server_settings("map_profiles:\n  0: night_tdm\n  3: duel_rules\n").unwrap();
        assert_eq!(settings.map_profiles.get(&0).map(String::as_str), Some("night_tdm"));
        assert_eq!(settings.map_profiles.get(&3).map(String::as_str), Some("duel_rules"));
        assert!(!settings.map_profiles.contains_key(&1));
    }

    #[cfg(feature = "map_profiles")]
    #[test]
    fn fixture_map_profile_resolves_to_existing_override() {
        // Same steps as the runtime path (yaml -> profile name -> override path), using the repo fixture.
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/test-configs");
        let yaml = std::fs::read_to_string(format!("{root}/ini-overrides/unchained.yaml")).unwrap();
        let settings = super::parse_server_settings(&yaml).unwrap();
        let profile = settings.map_profiles.get(&1).expect("fixture maps index 1");
        let paths = build_override_paths(root, &["Engine.ini".to_string()], &[(OverrideLayer::MapProfile, profile.clone())]);
        assert_eq!(paths.len(), 1);
        assert!(std::path::Path::new(&paths[0]).is_file(), "missing {}", paths[0]);
    }

    #[cfg(feature = "map_profiles")]
    #[test]
    fn parses_empty_or_unrelated_server_settings() {
        assert!(super::parse_server_settings("").unwrap().map_profiles.is_empty());
        assert!(super::parse_server_settings("other_setting: 5\n").unwrap().map_profiles.is_empty());
    }

    #[test]
    fn accepts_mod_tree_paths() {
        let path = "/Game/Mods/AgMods/SomeMod/Config/Engine.ini";
        assert!(super::is_mod_config_path(path));
    }

    #[test]
    fn detects_mod_root_from_object_path() {
        let object_path = "/Game/Mods/AgMods/SomeMod/SomeActor.SomeActor_C";
        assert!(super::object_path_is_mod_root(object_path));
    }

    #[test]
    fn detects_nested_mod_subfolders() {
        assert!(super::object_path_is_mod_root("/Game/Mods/AgMods/A/B/C/Deep.Deep_C"));
        assert!(super::object_path_is_mod_root("/game/mods/agmods/SomeMod/Default__SomeMod_C"));
    }

    #[test]
    fn ignores_sibling_and_other_mod_folders() {
        assert!(!super::object_path_is_mod_root("/Game/Mods/AgModsOther/SomeActor.SomeActor_C"));
        assert!(!super::object_path_is_mod_root("/Game/Mods/OtherMod/SomeActor.SomeActor_C"));
    }

    #[test]
    fn ignores_non_mod_object_paths() {
        let object_path = "/Game/Maps/ExampleMap/SomeActor.SomeActor_C";
        assert!(!super::object_path_is_mod_root(object_path));
    }
}
