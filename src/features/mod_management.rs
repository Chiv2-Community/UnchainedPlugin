use std::sync::Mutex;
#[allow(unused_imports)]
use widestring::U16CString;
use crate::features::Mod;
use crate::game::chivalry2::EChatType;
use crate::game::engine::{FActorSpawnParameters, FRotator, FText, TSoftClassPtr, get_assets_by_class};
use crate::game::unchained::{ArgonSDKModBase, DA_ModMarker_C, UModLoaderSettings_C, V2MarkerOffsets};
#[allow(unused_imports)]
use crate::resolvers::asset_registry::o_FNameCtorWchar;
use crate::tools::hook_globals::cli_args;
#[cfg(feature="mod_management")]
use crate::tools::hook_globals::globals;
use crate::ue::{AsUObject, FString, FVector, TArray, UClass, UObject};
use crate::resolvers::{asset_registry::*, asset_loading::*};
#[allow(unused_imports)]
use serde::Serialize;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use crate::game::engine::get_uobject_from_path;
use std::collections::{HashMap, HashSet};
use crate::resolvers::etc_hooks::*;
use crate::game::engine::ESpawnActorCollisionHandlingMethod::*;
use crate::resolvers::admin_control::o_FText_AsCultureInvariant;
use crate::resolvers::messages::o_BroadcastLocalizedChat;



// CREATE_COMMAND!(
//     "spawnmod",
//     [], 
//     "writes mods to json", 
//     {},
//     false,
//     |args| {
//         if args.is_empty() {
//             serror!("No path provided!");
//             return;
//         }
        


//         let mm_lock = || globals().mod_manager.lock().unwrap();
        
//         if let Some(mm) = mm_lock().as_ref() {
//             let mod_map = mm.get_available();
//             let mut mods: Vec<&Mod> = mod_map.values().collect();
//             mods.sort_by(|a, b| a.name.cmp(&b.name));
//             unsafe {
//                 // let obj = get_uobject_from_path(args.first().unwrap(), false);
//                 let id = args.first().unwrap().parse::<usize>().expect("Not a valid number");
//                 if id > mods.len() {
//                     sinfo!("{:#?}", mods);
//                     serror!("Invalid id {}", id);
//                     return;
//                 }
//                 let cur_mod = mods.get(id).expect("Not a valid mod");
//                 sinfo!("Spawning {} {}", id, cur_mod.name);
//                 let obj = get_uobject_from_path(cur_mod.object_path.as_str(), false);
//                 if !obj.is_null() {
//                     let mod_class = (&*obj).uobject_base_utility.uobject_base.class_private;
//                     mm.spawn_mod_actor(obj as *mut UClass);
//                 }
//                 else { serror!("Could not load from path") }
//             }
//             // let _ = mm.update_save_game();
//         }
//     }
// );

// CREATE_COMMAND!(
//     "listmods",
//     ["mods", "lsmods"], 
//     "Scans and lists all available and active mods", 
//     {},
//     false,
//     |args| {
//         use crate::{resolvers::unchained_integration::run_on_game_thread};
//         let mm_lock = || globals().mod_manager.lock().unwrap();

//         if mm_lock().as_ref().is_some_and(|mm| mm.get_available().is_empty()) {
//             let (tx, rx) = mpsc::channel();
            
//             run_on_game_thread(move || {
//                 if let Some(mm) = mm_lock().as_ref() {
//                     sinfo!("Starting scan!");
//                     mm.scan_asset_registry();
//                     let _ = tx.send(());
//                 }
//                 sinfo!("Ended scan!");
//             }); 
            
//             let _ = rx.recv();
//         } else {
//             if let Some(mm) = mm_lock().as_ref() {
//                 let _ = mm.scan_active_mod_actors();
//             }
//         }

//         if let Some(mm) = mm_lock().as_ref() {
//             mm.dump_to_console();
//         }     
//     }
// );

#[derive(Debug)]
pub struct ModManager {
    /// Static metadata lookup (Path -> Mod Metadata)
    registry: Mutex<HashMap<String, Mod>>,
    /// Set of active object paths
    active_paths: Mutex<HashSet<String>>,
}

const UE_INVALID_OBJECT_PATH_CHARS: &str = "\"' ,|&!~\n\r\t@#(){}[]=;^%$`";
const UE_INVALID_LONG_PACKAGE_CHARS: &str = "\\:*?\"<>|' ,.&!~\n\r\t@#;";

// Mirrors UE 4.25 FPackageName::IsValidObjectPath, with an explicit // check.
// The loader additionally requires an object/class suffix after the first '.'.
fn is_valid_object_path(path: &str) -> bool {
    if !path.starts_with('/') || path.is_empty() || path.contains("//") {
        return false;
    }

    let Some((package, object)) = path.split_once('.') else {
        return false;
    };

    if package.len() <= 1
        || package.ends_with('/')
        || package.contains('\0')
        || package
            .chars()
            .any(|c| UE_INVALID_LONG_PACKAGE_CHARS.contains(c))
    {
        return false;
    }

    if object.is_empty()
        || object.contains('\0')
        || object.ends_with(['.', ':'])
        || object.contains('/')
        || object
            .chars()
            .any(|c| UE_INVALID_OBJECT_PATH_CHARS.contains(c))
    {
        return false;
    }

    true
}

const MOD_MARKER_V1_CLASS: &str = "DA_ModMarker_C";
const MOD_MARKER_V2_CLASS: &str = "DA_ModMarker_V2_C";

/// Object paths of every asset of `class_name` in the Asset Registry. A failed
/// query means no marker of that class is installed, which is not an error.
unsafe fn marker_object_paths(class_name: &str) -> Vec<String> {
    let Ok(assets) = get_assets_by_class(class_name.into()) else {
        return Vec::new();
    };
    assets
        .as_slice()
        .iter()
        .filter(|asset| asset.ObjectPath.comparison_index.value != 0)
        .map(|asset| asset.ObjectPath.to_string())
        .collect()
}

/// Reads the mod metadata from a mod actor class's CDO. Returns the class path
/// (the registry key) and the entry, or `None` if the class or CDO is missing.
unsafe fn mod_from_class(class_ptr: *mut UClass) -> Option<(String, Mod)> {
    if class_ptr.is_null() {
        return None;
    }
    let cdo_new = CALL_ORIGINAL_SAFE!(GetDefaultObject(class_ptr, true)).ok()?;
    if cdo_new.is_null() {
        return None;
    }

    let cdo = &*(cdo_new as *mut ArgonSDKModBase);
    let class_obj = &(*class_ptr).ustruct.ufield.uobject;
    let asset_path = class_obj.uobject_base_utility.uobject_base.get_path_name(None);

    let mod_entry = Mod {
        name: cdo.mod_name.to_string(),
        organization: cdo.author.to_string(),
        version: cdo.mod_version.to_string(),
        object_path: asset_path.clone(),
    };
    Some((asset_path, mod_entry))
}

/// Loads `path` as an object of `object_class`; null if it does not exist or
/// is of another class.
fn load_object_of_class(object_class: *mut UClass, path: &str) -> Result<*mut UObject, String> {
    let wide_path = U16CString::from_str(path).map_err(|error| error.to_string())?;
    CALL_ORIGINAL_SAFE!(StaticLoadObject(
        object_class,
        std::ptr::null_mut(),
        wide_path.as_ptr(),
        std::ptr::null(),
        0,
        std::ptr::null_mut(),
        false,
        std::ptr::null_mut()
    ))
}

/// Combines both marker scans; a mod actor listed by both versions keeps its
/// V2 entry.
fn merge_marker_scans(v1: HashMap<String, Mod>, v2: HashMap<String, Mod>) -> HashMap<String, Mod> {
    let mut merged = v1;
    merged.extend(v2);
    merged
}

/// Bundled marker paths the registry query did not return. Bundled markers
/// ship as their own assets, so each should be among `known`.
fn missing_bundled_markers<'a>(bundled: &'a [String], known: &[String]) -> Vec<&'a str> {
    bundled
        .iter()
        .filter(|path| !known.iter().any(|k| k.eq_ignore_ascii_case(path)))
        .map(String::as_str)
        .collect()
}

// Validate syntax before passing launcher input to Unreal's string/path APIs.
fn enabled_mod_paths<'a>(
    mods: Option<&'a [String]>,
    mut class_exists: impl FnMut(&str) -> bool,
) -> Vec<&'a str> {
    mods.into_iter().flatten().filter_map(|path| {
        let valid_path = is_valid_object_path(path);
        if valid_path && class_exists(path) {
            Some(path.as_str())
        } else {
            swarn!("Skipping missing or invalid mod class: {:?}", path);
            None
        }
    }).collect()
}

impl ModManager {
    pub fn new() -> Self {
        Self {
            registry: Mutex::new(HashMap::new()),
            active_paths: Mutex::new(HashSet::new()),
        }
    }

    /// Get Mod base class for Actor lookup
    pub fn get_mod_base_class(&self) -> *mut UClass {
        let base_class_name = "/Game/Mods/ArgonSDK/Mods/ArgonSDKModBase.ArgonSDKModBase_C"; 
        let base_class_ptr = unsafe { get_uobject_from_path(base_class_name, false) as *mut UClass };

        match base_class_ptr.is_null() {
            true => std::ptr::null_mut(),
            false => base_class_ptr
        }
    }

    /// Populates the internal containers with mod info
    ///
    /// 1. Scans for mod markers (V1 `DA_ModMarker_C`, then V2
    ///    `DA_ModMarker_V2_C`) in the Asset Registry
    /// 2. Resolves each mod actor class referenced by the markers
    /// 3. For each actor, retrieves the CDO containing name/version
    ///
    /// A mod actor listed by both marker versions ends up as one entry, taken
    /// from the V2 marker.
    ///
    /// ## CAUTION
    /// Should run in game thread. Only stable in main menu otherwise
    pub fn scan_asset_registry(&self) {
        // TODO: check game thread here
        let v1 = unsafe { self.scan_v1_markers() };
        let v2 = unsafe { self.scan_v2_markers() };
        sinfo!("Found {} mod(s) in V1 markers, {} in V2 markers", v1.len(), v2.len());
        let registry_local = merge_marker_scans(v1, v2);

        sinfo!("Found {} Mods", registry_local.len());

        {
            let mut mods = self.registry.lock().expect("Lock poisoned");
            *mods = registry_local;
        }

        match self.scan_active_mod_actors() {
            Ok(_) => {},
            Err(e) => {serror!("Failed to scan active mods: {}", e);}
        };
    }

    /// Mods listed by deprecated V1 markers (`DA_ModMarker_C`), which hold hard
    /// references to their mod actor classes.
    unsafe fn scan_v1_markers(&self) -> HashMap<String, Mod> {
        let mut found: HashMap<String, Mod> = HashMap::new();
        for path in marker_object_paths(MOD_MARKER_V1_CLASS) {
            let mobj = get_uobject_from_path(&path, true);
            if mobj.is_null() { continue; }

            let mod_struct = mobj as *mut DA_ModMarker_C;
            for (mod_class_ref, _) in (&*mod_struct).mod_actors.iter() {
                if let Some((asset_path, mod_entry)) = mod_from_class(*mod_class_ref) {
                    found.insert(asset_path, mod_entry);
                }
            }
        }
        found
    }

    /// Mods listed by V2 markers (`DA_ModMarker_V2_C`), which hold soft class
    /// paths; each listed class is loaded to read its CDO, as V1 does implicitly.
    unsafe fn scan_v2_markers(&self) -> HashMap<String, Mod> {
        let mut found: HashMap<String, Mod> = HashMap::new();
        let marker_paths = marker_object_paths(MOD_MARKER_V2_CLASS);
        if marker_paths.is_empty() {
            return found;
        }
        let class_type = get_uobject_from_path("/Script/CoreUObject.Class", false) as *mut UClass;
        if class_type.is_null() {
            serror!("Class type not found; cannot resolve V2 marker mod classes");
            return found;
        }

        // Offsets are per class; every V2 marker normally shares one class.
        let mut offsets_by_class: HashMap<usize, Option<V2MarkerOffsets>> = HashMap::new();
        // Compact per-marker tokens, logged as one summary line instead of a
        // verbose line per marker.
        let mut marker_summary: Vec<String> = Vec::new();
        for path in &marker_paths {
            let marker = get_uobject_from_path(path, true);
            if marker.is_null() {
                swarn!("Could not load V2 marker {}", path);
                continue;
            }
            let marker_class = (*marker).uobject_base_utility.uobject_base.class_private;
            let offsets = *offsets_by_class.entry(marker_class as usize).or_insert_with(|| {
                match V2MarkerOffsets::resolve(marker_class) {
                    Ok(offsets) => Some(offsets),
                    Err(error) => {
                        serror!("Cannot read V2 markers of this class: {}", error);
                        None
                    }
                }
            });
            let Some(offsets) = offsets else { continue };

            let class_paths = offsets.mod_actor_classes(marker);
            let bundled_markers = offsets.bundled_markers(marker);

            let mod_id = offsets.mod_id(marker);
            let mod_version = offsets.mod_version(marker);
            let mut token = if mod_id.is_empty() {
                "<no ModId>".to_string()
            } else {
                mod_id
            };
            if !mod_version.is_empty() {
                token.push_str(&format!(" v{}", mod_version));
            }
            if !bundled_markers.is_empty() {
                token.push_str(&format!(" (+{} bundled)", bundled_markers.len()));
            }
            marker_summary.push(token);

            for bundled in missing_bundled_markers(&bundled_markers, &marker_paths) {
                swarn!("V2 marker {} bundles {}, which is not in the Asset Registry", path, bundled);
            }

            for class_path in class_paths {
                if !is_valid_object_path(&class_path) {
                    swarn!("V2 marker {} lists an invalid mod class path: {:?}", path, class_path);
                    continue;
                }
                match load_object_of_class(class_type, &class_path) {
                    Ok(class_ptr) if !class_ptr.is_null() => {
                        if let Some((asset_path, mod_entry)) = mod_from_class(class_ptr as *mut UClass) {
                            found.insert(asset_path, mod_entry);
                        } else {
                            swarn!("V2 marker {}: no CDO for mod class {}", path, class_path);
                        }
                    }
                    Ok(_) => swarn!("V2 marker {} lists a missing mod class: {}", path, class_path),
                    Err(error) => serror!("Cannot load mod class {}: {}", class_path, error),
                }
            }
        }
        if !marker_summary.is_empty() {
            sinfo!("V2 markers ({}): {}", marker_summary.len(), marker_summary.join(", "));
        }
        found
    }

    /// SCAN 2: Active Mod Actors (Entities currently in the World)
    /// Uses the provided ModActorStruct to find what is currently ticking
    pub fn scan_active_mod_actors(&self) -> Result<HashSet<String>, String> {
        let mod_base_class = self.get_mod_base_class();
        if mod_base_class.is_null() {
            return Err("Base class not found".to_string());
        }

        let world = match globals().world() {
            Some(world) => world,
            None => {
                swarn!("globals.world was None; skipping active mod actor scan");
                return Err("globals.world was None".to_string());
            }
        };

        let mut res_arr = TArray::<*mut UObject>::default();
        
        CALL_ORIGINAL_SAFE!(GetAllActorsOfClass(
            world.as_uobject(),
            mod_base_class,
            &mut res_arr
        ))?;

        let actors = res_arr.as_slice();
        
        let mut active_mods_local = HashSet::new();

        for &raw_ptr in actors {
            if raw_ptr.is_null() { continue; }
            
            unsafe {
                let class_ptr = (*raw_ptr).uobject_base_utility.uobject_base.class_private;
                if !class_ptr.is_null() {
                    let asset_path = (*class_ptr).ustruct.ufield.uobject
                        .uobject_base_utility.uobject_base
                        .get_path_name(None);
                    
                    active_mods_local.insert(asset_path);
                }
            }
        }
        {
            let mut active = self.active_paths.lock().map_err(|_| "Mutex poisoned")?;
            *active = active_mods_local.clone();
        }
        
        Ok(active_mods_local)
    }
    

    /// Spawn/Destroy Stubs
    ///
    /// Spawning needs the game thread; when called from another thread this
    /// queues itself and returns before the actor exists.
    pub unsafe fn spawn_mod_actor(&self, mod_class: *mut UClass) {
        use crate::resolvers::unchained_integration::{in_game_thread_job, run_on_game_thread};

        if mod_class.is_null() {
            serror!("mod class was null; skipping actor spawn");
            return;
        }
        if !in_game_thread_job() {
            let class_addr = mod_class as usize;
            run_on_game_thread(move || {
                if let Some(mm) = globals().mod_manager.lock().unwrap().as_ref() {
                    unsafe { mm.spawn_mod_actor(class_addr as *mut UClass) };
                }
            });
            return;
        }
        // (world: *mut UWorld, class: *mut UClass, position: *mut FVector, rotation: *mut FRotator, spawn_params: *mut FActorSpawnParameters)
        let world = match globals().world() {
            Some(world) => world,
            None => {
                swarn!("globals.world was None; skipping actor spawn");
                return;
            }
        };

        let mut loc: FVector = FVector { x: 0.0, y: 0.0, z: 0.0 };
        let mut rot: FRotator = FRotator::ZERO;
        let mut params: FActorSpawnParameters = FActorSpawnParameters::new().
            with_spawn_mode(AdjustIfPossibleButAlwaysSpawn);
        let new_actor = match CALL_ORIGINAL_SAFE!(SpawnActor(
            world,
            mod_class,
            &mut loc,
            &mut rot,
            &mut params
        )) {
            Ok(actor) => actor,
            Err(error) => {
                serror!("Cannot spawn mod actor: {}", error);
                return;
            }
        };

        unsafe {
            let actor_obj = new_actor as *mut UObject;
            if actor_obj.is_null() { serror!("Failed to spawn")}
            else {
                sinfo!("Spawned {}", (&*actor_obj).uobject_base_utility.uobject_base.name_private);
                self.broadcast_spawn_confirmation();
            }
        }

        if let Some(reg) = globals().registration.lock().unwrap().as_ref() {
            let _ = self.scan_active_mod_actors();
            reg.set_mods(self.get_active_mod_metadata());
        }
    }

    /// Best-effort admin chat after a spawn; every step may be unavailable
    /// (no world, no game mode in the menu, detour not installed).
    fn broadcast_spawn_confirmation(&self) {
        let Some(world) = globals().world() else {
            swarn!("globals.world missing; skipping spawn confirmation chat");
            return;
        };
        let mut txt = FText::default();
        let mut message = FString::from("Spawned new Mod from console");
        let converted = CALL_ORIGINAL_SAFE!(FText_AsCultureInvariant(&mut txt, &mut message));
        // The engine may take ownership of the string (by-value overload), so
        // don't free it a second time; at worst this leaks a short string.
        std::mem::forget(message);
        let text = match converted {
            Ok(text) if !text.is_null() => text,
            _ => {
                swarn!("FText conversion unavailable; skipping spawn confirmation chat");
                return;
            }
        };
        let game_mode = match CALL_ORIGINAL_SAFE!(GetTBLGameMode(world.as_uobject())) {
            Ok(game_mode) if !game_mode.is_null() => game_mode,
            _ => {
                swarn!("No game mode; skipping spawn confirmation chat");
                return;
            }
        };
        if let Err(error) = CALL_ORIGINAL_SAFE!(BroadcastLocalizedChat(game_mode, text, EChatType::Admin)) {
            swarn!("Cannot broadcast spawn confirmation: {}", error);
        }
    }

    pub fn destroy_mod_actor(&self, _actor_ptr: *mut UObject) {
        // TODO: implement
    }

    /*
    
    --server-mods /Game/Mods/AgMods/GiantSlayers/GiantSlayers.GiantSlayers_C,/Game/Mods/AgMods/ManlyMen/ManlyMen.ManlyMen_C,/Game/Mods/AgMods/ChatHooks/ChatHooks.ChatHooks_C,/Game/Mods/AgMods/PropHuntOnline/PropHuntOnline.PropHuntOnline_C
    
     */
    /// Replace saved mod selection with validated CLI classes, on the game thread.
    ///
    /// Also seeds the active set with the server's configured mods: the Asset
    /// Registry scan only populates *available* mods, and a live-world actor
    /// scan is not reliable this early during init, so `--server-mods` is the
    /// authoritative source of what this server runs for registration.
    pub fn update_save_game(&self) {
        crate::sinfo!("Updating ModLoader settings: {:?}", cli_args().saved_dir_suffix);
        let settings_file = "ModLoader";
        let mut settings_fstring = FString::from(settings_file);
        let save_game_ptr = TRY_CALL_ORIGINAL!(LoadGameFromSlot(&mut settings_fstring, 0)) ;
        if save_game_ptr.is_null() {
            serror!("save game is null");
            return;
        }
        let save_game = unsafe {&mut *(save_game_ptr as *mut UModLoaderSettings_C)};
        // sinfo!("Save Game:   {:#?}", save_game);
        sinfo!("Enable Bots: {}", save_game.b_enable_bots);
        sinfo!("PlayerBots:  {}", save_game.b_use_player_bots);
        sinfo!("Bot count:   {}", save_game.num_player_bots);
        // Loading with a Class filter rejects ordinary assets as well as stale paths.
        let server_mods = cli_args().server_mods.as_deref();
        let class_type = if server_mods.is_some_and(|mods| !mods.is_empty()) {
            let class_type = unsafe { get_uobject_from_path("/Script/CoreUObject.Class", false) };
            if class_type.is_null() {
                serror!("Class type not found; cannot validate any mods");
            }
            class_type
        } else {
            std::ptr::null_mut()
        };
        let paths = enabled_mod_paths(server_mods, |path| {
            if class_type.is_null() {
                return false;
            }
            match load_object_of_class(class_type as *mut UClass, path) {
                Ok(object) => !object.is_null(),
                Err(error) => {
                    serror!("Cannot validate mod {}: {}", path, error);
                    false
                }
            }
        });
        // Both lists belong to the previous launch. Empty CLI input must also
        // remove the cached active mods, rather than allowing them to return.
        save_game.enabled_mods.clear();
        save_game.last_active_mods.clear();
        let mut enabled = Vec::with_capacity(paths.len());
        for path in paths {
            sinfo!("Enabling mod {}", path);
            save_game.enabled_mods.push(TSoftClassPtr::from_path(path));
            enabled.push(path.to_string());
        }
        // The configured server mods are the active set for registration; a
        // live-world actor scan is unreliable at init (world may be None and
        // the mod actors are not spawned yet).
        self.set_active_paths(enabled);
        let res = TRY_CALL_ORIGINAL!(SaveGameToSlot(save_game_ptr, &mut settings_fstring, 0));
        if res {
            sinfo!("Modloader settings saved: {res}");
        }
        else {
            serror!("Failed to save Modloader settings");

        }
    }

    /// Marks the given mod class paths as the active set. Used to advertise the
    /// server's configured mods (`--server-mods`) for registration without
    /// depending on a live-world actor scan, which is not reliable during init.
    pub fn set_active_paths(&self, paths: impl IntoIterator<Item = String>) {
        let mut active = self.active_paths.lock().unwrap();
        *active = paths.into_iter().collect();
    }

    pub fn dump_to_console(&self) {
        let registry = self.registry.lock().unwrap();
        let active = self.active_paths.lock().unwrap();

        sinfo!("--- Registered Mods ({}) ---", registry.len());
        let mut sorted_mods: Vec<&Mod> = registry.values().collect();
        sorted_mods.sort_by(|a, b| a.name.cmp(&b.name));
        for (idx, m) in sorted_mods.iter().enumerate() {
            let is_active = active.contains(&m.object_path);
            let status_tag = if is_active {
                "[ Active ]"
            } else {
                "[Inactive]"
            };
            sinfo!(
                "[{:02}] {status_tag} \"{}\" v{} by {}", 
                idx, m.name, m.version, m.organization
            );
        }
        sinfo!("----------------------------");
    }

    pub fn serialize_registry(&self, file_path: &str) -> Result<(), Box<dyn std::error::Error>> 
    {
        let path = Path::new(file_path);
        let file = File::create(path)?;
        
        let writer = BufWriter::new(file);

        let reg = self.registry.lock().unwrap().clone();
        serde_json::to_writer_pretty(writer, &reg)?;

        Ok(())
    }
    
    /// Returns a Vec of Mod structs for all currently active mods
    /// Useful for Server Registration
    pub fn get_active_mod_metadata(&self) -> Vec<Mod> {
        let registry = self.registry.lock().unwrap();
        let active: std::sync::MutexGuard<'_, HashSet<String>> = self.active_paths.lock().unwrap();

        active.iter()
            .filter_map(|path| registry.get(path).cloned())
            .collect()
    }

    pub fn reset_active_mods(&self, rescan: bool) {
        self.active_paths.lock().unwrap().clear();
        if rescan {
            let _ = self.scan_active_mod_actors();
        }
    }

    /// Get available mods
    pub fn get_available(&self) -> HashMap<String, Mod> {
        self.registry.lock().unwrap().clone()
    }

    /// Get currently enabled mods
    pub fn get_active(&self) -> HashMap<String, Mod> {
        let active = self.active_paths.lock().unwrap();
        let registry = self.registry.lock().unwrap();
        registry.iter()
            .filter(|&(k,_v)| active.contains(k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_unreal_object_paths() {
        for path in [
            "/Game/Mods/Example.Example_C",
            "/Game/Mods/Foo.Bar.Baz_C",
        ] {
            assert!(is_valid_object_path(path), "expected valid path: {path}");
        }

        for path in [
            "",
            "Game/Mods/Example.Example_C",
            "/Game//Mods/Example.Example_C",
            "/Game/Mods/Example",
            "/Game/Mods/.Example_C",
            "/Game/Mods/Example.",
            "/Game/Mods/Bad;Name.Bad_C",
            "/Game/Mods/Bad\0Name.Bad_C",
        ] {
            assert!(!is_valid_object_path(path), "expected invalid path: {path:?}");
        }
    }

    #[test]
    fn absent_or_empty_server_mods_enable_nothing() {
        let unexpected_lookup = |_: &str| panic!("empty input must not resolve classes");
        assert!(enabled_mod_paths(None, unexpected_lookup).is_empty());
        assert!(enabled_mod_paths(Some(&[]), unexpected_lookup).is_empty());
    }

    #[test]
    fn invalid_and_missing_classes_are_skipped() {
        let mods: Vec<String> = [
            "/Game/Mods/Example.Example_C",
            "/Game/Mods/Old.Old_C",
            "", "/Game/Mods/NoClass", "/Game/Mods/Empty.",
            "/Game/Mods/Bad.Bad\0_C",
            "/Game/Mods/Other.Other_C",
        ].into_iter().map(String::from).collect();
        let mut resolved = Vec::new();
        let paths = enabled_mod_paths(Some(&mods), |path| {
            resolved.push(path.to_owned());
            !path.contains("Old")
        });
        assert_eq!(paths, [mods[0].as_str(), mods[6].as_str()]);
        assert_eq!(resolved, [mods[0].clone(), mods[1].clone(), mods[6].clone()]);
    }

    #[test]
    fn v2_marker_entry_replaces_v1_entry_for_the_same_class() {
        let entry = |version: &str, path: &str| Mod {
            name: "Example".into(), organization: "Tests".into(),
            version: version.into(), object_path: path.into(),
        };
        let shared = "/Game/Mods/Example/Example.Example_C";
        let v1_only = "/Game/Mods/Old/Old.Old_C";
        let v1 = HashMap::from([
            (shared.to_string(), entry("1.0.0", shared)),
            (v1_only.to_string(), entry("0.1.0", v1_only)),
        ]);
        let v2 = HashMap::from([(shared.to_string(), entry("2.0.0", shared))]);
        let merged = merge_marker_scans(v1, v2);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[shared].version, "2.0.0");
        assert_eq!(merged[v1_only].version, "0.1.0");
    }

    #[test]
    fn reports_only_bundled_markers_missing_from_the_registry() {
        let known = vec!["/Game/Mods/A/A_ModMarker.A_ModMarker".to_string()];
        let bundled = vec![
            "/Game/Mods/a/A_ModMarker.A_ModMarker".to_string(),
            "/Game/Mods/B/B_ModMarker.B_ModMarker".to_string(),
        ];
        assert_eq!(missing_bundled_markers(&bundled, &known), vec![bundled[1].as_str()]);
    }

    #[test]
    fn owned_collections_preserve_mod_queries_and_reset() {
        let manager = ModManager::new();
        let metadata = Mod {
            name: "Example".into(), organization: "Tests".into(),
            version: "1.0.0".into(), object_path: "/Game/Mods/Example".into(),
        };
        manager.registry.lock().unwrap().insert(metadata.object_path.clone(), metadata.clone());
        manager.active_paths.lock().unwrap().insert(metadata.object_path.clone());
        assert_eq!(manager.get_available().get(&metadata.object_path), Some(&metadata));
        assert_eq!(manager.get_active().get(&metadata.object_path), Some(&metadata));
        assert_eq!(manager.get_active_mod_metadata(), vec![metadata]);
        manager.reset_active_mods(false);
        assert!(manager.get_active().is_empty());
        assert!(manager.get_active_mod_metadata().is_empty());
        assert_eq!(manager.get_available().len(), 1);
    }
}
