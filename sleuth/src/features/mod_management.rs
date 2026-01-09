use std::hash::{DefaultHasher, Hash, Hasher};
use std::os::raw::c_void;
use std::sync::{Arc, Mutex, mpsc};
use widestring::U16CString;
use crate::features::Mod;
use crate::game::chivalry2::EChatType;
use crate::game::engine::{FActorSpawnParameters, FRotator, FText, TSoftClassPtr, get_assets_by_class};
use crate::game::unchained::{ArgonSDKModBase, DA_ModMarker_C, UModLoaderSettings_C};
use crate::resolvers::asset_registry::{FAssetData, TScriptInterface};
#[cfg(feature="mod_management")]
use crate::tools::hook_globals::globals;
use crate::{ serror, sinfo};
use crate::ue::{FName, FNameEntryId, FString, FVector, TArray, TMap, UClass, UObject};
use crate::resolvers::{asset_registry::*, asset_loading::*};
use serde::Serialize;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use crate::game::engine::get_uobject_from_path;
use std::collections::{HashMap, HashSet};
use crate::resolvers::etc_hooks::*;
use crate::game::engine::ESpawnActorCollisionHandlingMethod::*;
use crate::resolvers::admin_control::o_FText_AsCultureInvariant;
use crate::resolvers::messages::o_BroadcastLocalizedChat;
use crate::commands::{CommandResult, ConsoleCommand};

#[macro_export]
macro_rules! check_main_thread {
    () => {
        if std::thread::current().id() == globals().main_thread_id {
            $crate::sinfo!("IS IN MAIN THREAD");
        }
        else {
            $crate::serror!("IS NOT IN MAIN THREAD");
        }
    }
}
// in my_engine/src/commands.rs
use sleuth_macros::command;

#[command(name = "mod", sub = "dump", desc = "Dumps mod list. Path is optional")]
fn dump_mods(path: Option<String>) -> CommandResult {
    let target: &str = path.as_deref().unwrap_or("ingame_mod_registry.json");
    let mm_lock = || globals().mod_manager.lock().unwrap();
    if let Some(mm) = mm_lock().as_ref() {
        let _ = mm.serialize_registry(target);
    }
    sinfo!(f; "Registry saved to \'{}\'", target);
    Ok(())
}


#[command(name="mod", sub="list", alias="lsmods", desc="Spawn entity")]
fn list_mods() -> CommandResult {
    use crate::{resolvers::unchained_integration::run_on_game_thread};
    let mm_lock = || globals().mod_manager.lock().unwrap();

    if mm_lock().as_ref().is_some_and(|mm| mm.get_available().is_empty()) {
        let (tx, rx) = mpsc::channel();
        sinfo!(f; "Starting scan!");
        
        run_on_game_thread(move || {
            if let Some(mm) = mm_lock().as_ref() {
                mm.scan_asset_registry();
                let _ = tx.send(());
            }
        }); 
        let _ = rx.recv();
    } else {
        if let Some(mm) = mm_lock().as_ref() {
        sinfo!(f; "Getting list!");
            let _ = mm.scan_active_mod_actors();
        }
    }

    if let Some(mm) = mm_lock().as_ref() {
        mm.dump_to_console();
    } 
    Ok(())
}

// CREATE_COMMAND!(
//     "spawnmod",
//     [], 
//     "writes mods to json", 
//     {},
//     false,
//     |args| {
//         if args.is_empty() {
//             serror!(f; "No path provided!");
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
//                     sinfo!(f; "{:#?}", mods);
//                     serror!(f; "Invalid id {}", id);
//                     return;
//                 }
//                 let cur_mod = mods.get(id).expect("Not a valid mod");
//                 sinfo!(f; "Spawning {} {}", id, cur_mod.name);
//                 let obj = get_uobject_from_path(cur_mod.object_path.as_str(), false);
//                 if !obj.is_null() {
//                     let mod_class = (&*obj).uobject_base_utility.uobject_base.class_private;
//                     mm.spawn_mod_actor(obj as *mut UClass);
//                 }
//                 else { serror!(f; "Could not load from path") }
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
//                     sinfo!(f; "Starting scan!");
//                     mm.scan_asset_registry();
//                     let _ = tx.send(());
//                 }
//                 sinfo!(f; "Ended scan!");
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
    registry: Arc<Mutex<HashMap<String, Mod>>>,
    /// Set of active object paths
    active_paths: Arc<Mutex<HashSet<String>>>,
}

impl ModManager {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(Mutex::new(HashMap::new())),
            active_paths: Arc::new(Mutex::new(HashSet::new())),
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
    /// 1. Scans for mod markers in Asset Registsry
    /// 2. Retrieves AssetData for each mod class referenced by markers
    /// 3. For each actor, retrieves the CDO containing name/version
    /// 
    /// ## CAUTION
    /// Should run in game thread. Only stable in main menu otherwise
    pub fn scan_asset_registry(&self) {
        // TODO: check game thread here
        let mut registry_local: HashMap<String, Mod> = HashMap::new();
        let cmd = "DA_ModMarker_C"; 
        
        let res_arr = unsafe { get_assets_by_class(cmd.into()).expect("failed") };
        
        for asset in res_arr.as_slice() {
            if asset.ObjectPath.comparison_index.value == 0 {
                continue;
            }

            let path = asset.ObjectPath.to_string();

            let mobj = unsafe { get_uobject_from_path(&path, true) };
            if mobj.is_null() { continue; }

            let mod_struct = mobj as *mut DA_ModMarker_C;
            
            unsafe {
                for (mod_class_ref, _) in (&*mod_struct).mod_actors.iter() {
                    let class_ptr = *mod_class_ref;
                    if class_ptr.is_null() { continue; }

                    // let cdo_new = match (&*class_ptr).class_default_object.is_null() {
                    //     true => TRY_CALL_ORIGINAL!(GetDefaultObject(class_ptr, true)),
                    //     false =>(&*class_ptr).class_default_object as *mut UObject
                    // };

                    let cdo_new = TRY_CALL_ORIGINAL!(GetDefaultObject(class_ptr, true));
                    if cdo_new.is_null() { continue; }

                    let cdo = &*(cdo_new as *mut ArgonSDKModBase);
                    let class_obj = &(*class_ptr).ustruct.ufield.uobject;
                    let asset_path = class_obj.uobject_base_utility.uobject_base.get_path_name(None);
                    
                    let mod_entry = Mod { 
                        name: cdo.mod_name.to_string(),
                        organization: cdo.author.to_string(),
                        version: cdo.mod_version.to_string(),
                        object_path: asset_path.clone(),
                    };

                    registry_local.insert(asset_path, mod_entry);
                }
            }
        }
        
        {
            let mut mods = self.registry.lock().expect("Lock poisoned");
            *mods = registry_local;
        }
        
        self.scan_active_mod_actors().expect("Failed to scan active mods");
    }

    /// SCAN 2: Active Mod Actors (Entities currently in the World)
    /// Uses the provided ModActorStruct to find what is currently ticking
    pub fn scan_active_mod_actors(&self) -> Result<HashSet<String>, String> {
        let mod_base_class = self.get_mod_base_class();
        if mod_base_class.is_null() {
            return Err("Base class not found".to_string());
        }

        let mut res_arr = TArray::<*mut UObject>::default();
        
        CALL_ORIGINAL_SAFE!(GetAllActorsOfClass(
            globals().world().expect("No world found") as *const UObject,
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
    pub fn spawn_mod_actor(&self, mod_class: *mut UClass) {
        // (world: *mut c_void, class: *mut UClass, position: *mut FVector, rotation: *mut FRotator, spawn_params: *mut FActorSpawnParameters)
        let mut loc: FVector = FVector { x: 0.0, y: 0.0, z: 0.0 };
        let mut rot: FRotator = FRotator::ZERO;
        let mut params: FActorSpawnParameters = FActorSpawnParameters::new().
            with_spawn_mode(AdjustIfPossibleButAlwaysSpawn);
        let new_actor = CALL_ORIGINAL!(SpawnActor(
            globals().world().unwrap(),
            mod_class,
            &mut loc,
            &mut rot,
            &mut params
        ));

        unsafe {
            let actor_obj = new_actor as *mut UObject;
            if actor_obj.is_null() { crate::serror!(f; "Failed to spawn")}
            else { 
                crate::sinfo!(f; "Spawned {}", (&*actor_obj).uobject_base_utility.uobject_base.name_private);
                let mut txt = FText::default();
                
                if let Some(world) = globals().world() {
                    let settings_file = "Spawned new Mod from console";
                    let mut settings_fstring = FString::from(settings_file);
                    let res = unsafe { TRY_CALL_ORIGINAL!(FText_AsCultureInvariant(&mut txt, &mut settings_fstring)) } as *mut FText;
                    let game_mode = TRY_CALL_ORIGINAL!(GetTBLGameMode(world));
                    TRY_CALL_ORIGINAL!(BroadcastLocalizedChat(game_mode, res, EChatType::Admin));
                }
            }
        }

        if let Some(reg) = globals().registration.lock().unwrap().as_ref() {
            let _ = self.scan_active_mod_actors();
            reg.set_mods(self.get_active_mod_metadata());
        }
    }

    pub fn destroy_mod_actor(&self, actor_ptr: *mut UObject) {
        // TODO: implement
    }

    /*
    
    --server-mods /Game/Mods/AgMods/GiantSlayers/GiantSlayers.GiantSlayers_C,/Game/Mods/AgMods/ManlyMen/ManlyMen.ManlyMen_C,/Game/Mods/AgMods/ChatHooks/ChatHooks.ChatHooks_C,/Game/Mods/AgMods/PropHuntOnline/PropHuntOnline.PropHuntOnline_C
    
     */
    pub fn update_save_game(&self) {
        let settings_file = "ModLoader";
        let mut settings_fstring = FString::from(settings_file);
        let save_game_ptr = unsafe { TRY_CALL_ORIGINAL!(LoadGameFromSlot(&mut settings_fstring, 0)) } ;
        if save_game_ptr.is_null() {
            serror!(f; "save game is null");
            return;
        }
        let save_game = unsafe {&mut *(save_game_ptr as *mut UModLoaderSettings_C)};
        // FIXME: apply mods from cli
        save_game.enabled_mods.clear();
        if let Some(mod_list) = globals().cli_args.server_mods.as_ref() {
            for Mod in mod_list.iter() {  
                if let Some(name_short) = Mod.rsplit_once('.').map(|(_, n)| n) {                    
                    let soft_ptr = TSoftClassPtr::from_path(Mod.as_str());
                    sinfo!(f; "Enabling mod {}", name_short);
                    save_game.enabled_mods.push(soft_ptr);
                }
            }      
        }  
        let res = unsafe { TRY_CALL_ORIGINAL!(SaveGameToSlot(save_game_ptr as *mut c_void, &mut settings_fstring, 0)) };
        sinfo!(f; "Game Saved: {res}");
    }

    pub fn dump_to_console(&self) {
        let registry = self.registry.lock().unwrap();
        let active = self.active_paths.lock().unwrap();

        println!("\n--- Registered Mods ({}) ---", registry.len());
        let mut sorted_mods: Vec<&Mod> = registry.values().collect();
        sorted_mods.sort_by(|a, b| a.name.cmp(&b.name));
        for (idx, m) in sorted_mods.iter().enumerate() {
            let is_active = active.contains(&m.object_path);
            let status_tag = if is_active {
                "\x1b[32m[ Active ]\x1b[0m " // Green
            } else {
                "\x1b[90m[Inactive]\x1b[0m " // Grey
            };
            println!(
                "[{:02}] {status_tag} \"{}\" v{} by {}", 
                idx, m.name, m.version, m.organization
            );
        }
        println!("----------------------------\n");
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
            self.scan_active_mod_actors();
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
            .filter(|&(k,v)| active.contains(k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}