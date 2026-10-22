use std::cell::Cell;

use crate::{
    game::{
        chivalry2::ATBLGameMode,
        engine::UWorld,
    },
    tools::hook_globals::globals,
};

const DEFAULT_MOD_ROOT: &str = "/Game/Mods/AgMods";
const MODS_TREE_PREFIX: &str = "/Game/Mods/";
const MAP_ROTATION_DIR: &str = "map-rotation-config";

#[cfg(feature = "config_overrides_verbose_hooks")]
macro_rules! config_debug {
    ($($arg:tt)*) => {
        crate::sdebug!($($arg)*);
    };
}

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

fn config_override_roots() -> Vec<String> {
    // We treat the default AgMods tree as the canonical mod root and then merge in any
    // additional mod roots supplied via the CLI. This gives us one place to answer: "is this
    // config path relevant to a mod override?" without scattering string checks everywhere.
    let mut roots = vec![DEFAULT_MOD_ROOT.to_string()];

    #[cfg(feature = "config_overrides_verbose_hooks")]
    config_debug!("🧩 [config-overrides] default roots: {:?}", roots);

    if let Some(cli) = crate::tools::hook_globals::CLI_ARGS.get() {
        if let Some(mods) = cli.server_mods.as_ref() {
            for mod_path in mods {
                let cleaned = mod_path.replace('\\', "/").trim().to_string();
                if let Some(suffix) = cleaned.strip_prefix(MODS_TREE_PREFIX) {
                    let root = format!("{MODS_TREE_PREFIX}{}", suffix.split('/').next().unwrap_or_default());
                    if !root.is_empty() && !roots.iter().any(|existing| existing == &root) {
                        roots.push(root);
                    }
                }
            }
        }
    }

    let roots = roots.into_iter().map(|path| path.replace('\\', "/")).collect::<Vec<_>>();
    #[cfg(feature = "config_overrides_verbose_hooks")]
    config_debug!("🧩 [config-overrides] resolved mod roots: {:?}", roots);
    roots
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

    if !is_ue_config {
        #[cfg(feature = "config_overrides_verbose_hooks")]
        config_debug!("🚫 [config-overrides] not a UE config file: {}", normalized);
        return false;
    }

    let is_match = config_override_roots()
        .into_iter()
        .map(|root| root.to_ascii_lowercase())
        .any(|root| lowered.contains(root.as_str()));

    #[cfg(feature = "config_overrides_verbose_hooks")]
    config_debug!("{} [config-overrides] mod-config check for {} -> {}", if is_match { "✅" } else { "⚠️" }, normalized, is_match);
    is_match
}

pub fn object_path_is_mod_root(object_path: &str) -> bool {
    // Some LoadConfig calls do not carry a useful filename, but the object path still tells us whether
    // the asset belongs to a mod hierarchy. That is enough to decide whether the override loader is
    // relevant even when the filename itself is generic or empty.
    let normalized = object_path.replace('\\', "/");
    let lowered = normalized.to_ascii_lowercase();

    let is_match = config_override_roots()
        .into_iter()
        .map(|root| root.to_ascii_lowercase())
        .any(|root| lowered.contains(root.as_str()));

    #[cfg(feature = "config_overrides_verbose_hooks")]
    config_debug!("{} [config-overrides] object-path mod check for {} -> {}", if is_match { "✅" } else { "⚠️" }, normalized, is_match);
    is_match
}

pub fn build_profile_override_path(profile_root: &str, map_slot: &str, config_name: &str) -> String {
    // This function builds the per-map override path under the server profile directory. The
    // directory layout is intentionally simple: <profile>/map-rotation-config/<map-name-or-index>/<Config>.ini.
    // We normalize both separators and trimming so the generated path matches what the engine expects.
    let clean_root = profile_root.trim_end_matches(['/', '\\']);
    let clean_slot = map_slot.trim_matches(['/', '\\']);
    let clean_name = config_name.trim_matches(['/', '\\']);
    format!("{clean_root}/{MAP_ROTATION_DIR}/{clean_slot}/{clean_name}").replace('\\', "/")
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

pub fn current_map_rotation_index() -> Option<String> {
    // The match rotation index is the second override key. It is useful when we want to target a slot
    // rather than a specific map name, especially during rotation or when map names are not stable.
    let world = globals().world()?;
    let world_ref = unsafe { &*(world as *mut UWorld) };
    let game_mode = unsafe { (world_ref.authority_game_mode as *mut ATBLGameMode).as_ref() }?;
    let index = game_mode.map_list_index;
    // This index is the rotation slot currently active in the match, so it is a useful
    // second-tier override source after the map name itself.
    if index >= 0 {
        return Some(index.to_string());
    }
    None
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

    if !has_mod_context {
        #[cfg(feature = "config_overrides_verbose_hooks")]
        config_debug!("🚫 [config-overrides] skip override resolution (no mod context): original={:?}, object={:?}", original_path, object_path);
        return vec![];
    }

    let profile_root = match crate::tools::hook_globals::CLI_ARGS.get().and_then(|cli| cli.server_profile_location.as_deref()) {
        Some(root) => root,
        None => {
            #[cfg(feature = "config_overrides_verbose_hooks")]
            config_debug!("⚠️ [config-overrides] missing server_profile_location, cannot resolve overrides");
            return vec![];
        }
    };

    let mut candidates = Vec::new();

    if let Some(name) = original_path {
        let file_name = name.rsplit('/').next().unwrap_or(name).trim();
        #[cfg(feature = "config_overrides_verbose_hooks")]
        config_debug!("🧭 [config-overrides] original_parsed={} original={}", file_name, name);
        if !file_name.is_empty() {
            candidates.push(file_name.to_string());
            #[cfg(feature = "config_overrides_verbose_hooks")]
            config_debug!("🧭 [config-overrides] original added to candidates");
        }
    }

    if candidates.is_empty() {
        for fallback in ["Engine.ini", "Game.ini", "DefaultEngine.ini"] {
            candidates.push(fallback.to_string());
        }
    }

    if let Some(path) = object_path {
        let object_file = path.rsplit('/').next().unwrap_or(path).trim();
        if !object_file.is_empty() && object_file.contains('.') && !candidates.iter().any(|candidate| candidate.eq_ignore_ascii_case(object_file)) {
            candidates.push(object_file.to_string());
        }
    }

    #[cfg(feature = "config_overrides_verbose_hooks")]
    config_debug!("🧭 [config-overrides] profile_root={} candidates={:?}", profile_root, candidates);

    let mut paths = Vec::new();
    let mut seen = std::collections::HashSet::new();
    paths.push(original_path.map(|name| name.replace('\\', "/")).unwrap_or_default()); // always do the Engine.ini from saved first

    for config_name in candidates {
        let normalized_original = original_path.map(|name| name.replace('\\', "/")).unwrap_or_default();

        if let Some(map_name) = current_map_rotation_folder_name() {
            let path = build_profile_override_path(profile_root, &map_name, &config_name);
            if !normalized_original.is_empty() && path == normalized_original {
                #[cfg(feature = "config_overrides_verbose_hooks")]
                config_debug!("⏭️ [config-overrides] skipping duplicate map-name path: {}", path);
                continue;
            }
            if seen.insert(path.clone()) {
                #[cfg(feature = "config_overrides_verbose_hooks")]
                config_debug!("✅ [config-overrides] added map-name override path: {}", path);
                paths.push(path);
            }
        }

        if let Some(map_index) = current_map_rotation_index() {
            let path = build_profile_override_path(profile_root, &map_index, &config_name);
            if !normalized_original.is_empty() && path == normalized_original {
                #[cfg(feature = "config_overrides_verbose_hooks")]
                config_debug!("⏭️ [config-overrides] skipping duplicate map-index path: {}", path);
                continue;
            }
            if seen.insert(path.clone()) {
                #[cfg(feature = "config_overrides_verbose_hooks")]
                config_debug!("✅ [config-overrides] added map-index override path: {}", path);
                paths.push(path);
            }
        }
    }

    #[cfg(feature = "config_overrides_verbose_hooks")]
    config_debug!("🔍 [config-overrides] final override list: {:?}", paths);
    paths
}

#[cfg(test)]
mod tests {
    use super::build_profile_override_path;

    #[test]
    fn builds_per_map_override_path_for_engine_ini() {
        let path = build_profile_override_path(r"C:\Profiles\Server", "TDM_QuarryKP", "Engine.ini");
        assert_eq!(path, "C:/Profiles/Server/map-rotation-config/TDM_QuarryKP/Engine.ini");
    }

    #[test]
    fn builds_per_map_override_path_for_game_ini() {
        let path = build_profile_override_path(r"C:/Profiles/Server/", "MapRotationSlot1", "Game.ini");
        assert_eq!(path, "C:/Profiles/Server/map-rotation-config/MapRotationSlot1/Game.ini");
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
    fn ignores_non_mod_object_paths() {
        let object_path = "/Game/Maps/ExampleMap/SomeActor.SomeActor_C";
        assert!(!super::object_path_is_mod_root(object_path));
    }
}
