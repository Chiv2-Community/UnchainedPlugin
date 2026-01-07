use std::hash::{DefaultHasher, Hash, Hasher};
use std::os::raw::c_void;
use std::sync::{Arc, Mutex, mpsc};
use widestring::U16CString;
use crate::features::Mod;
use crate::features::commands::ConsoleCommandHandler;
use crate::resolvers::asset_registry::{FAssetData, TScriptInterface};
#[cfg(feature="mod_management")]
use crate::tools::hook_globals::globals;
use crate::{CREATE_COMMAND, sinfo};
use crate::ue::{FName, FNameEntryId, FString, TArray, TMap, UClass, UObject};
use crate::resolvers::{asset_registry::*};

// Hashes for TMap
use crate::ue::UEHash;
impl UEHash for *mut crate::ue::UClass {
    fn ue_hash(&self) -> u32 {
        let mut hasher = DefaultHasher::new();
        (*self as usize).hash(&mut hasher);
        (hasher.finish() & 0xFFFF_FFFF) as u32
    }
}
impl UEHash for *mut crate::ue::UObject {
    fn ue_hash(&self) -> u32 {
        let mut hasher = DefaultHasher::new();
        (*self as usize).hash(&mut hasher);
        (hasher.finish() & 0xFFFF_FFFF) as u32
    }
}

#[repr(C)]
pub struct ModActorStruct {
    _private: [u8; 0x30], 
    pub mod_actors: TMap<*mut UClass, FString>,    // Offset 0x0030
    pub custom_objects: TMap<*mut UObject, FString>, // Offset 0x0080
}

use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub struct ModManager {
    /// Static metadata lookup (Path -> Mod Metadata)
    registry: Arc<Mutex<HashMap<String, Mod>>>,
    /// Set of active object paths
    active_paths: Arc<Mutex<HashSet<String>>>,
}

#[repr(C)]
pub struct ArgonSDKModBase {
    pub actor_padding: [u8; 0x258],
    // 0x0258
    pub uber_graph_frame: u64, 
    // 0x0260
    pub default_scene_root: *mut c_void, 
    // 0x0268
    pub mod_name: FString,
    // 0x0278
    pub mod_version: FString,
    // 0x0288
    pub mod_description: FString,
    // 0x0298
    pub author: FString,
    // 0x02A8
    pub b_silent_load: bool,
    pub padding_0: [u8; 0x7], // 0x02A9
    // 0x02B0
    pub mod_repo_url: FString,
    // 0x02C0
    pub duplicate: bool,
    pub b_enable_by_default: bool,
    pub b_show_in_gui: bool,
    pub b_online_only: bool,
    pub b_host_only: bool,
    pub b_allow_on_frontend: bool,
    pub padding_1: [u8; 0x2], // 0x02C6
    // 0x02C8
    pub mod_version_repl: FString,
    // 0x02D8
    // pub ingame_mod_menu_widgets: TMap<FString, *mut c_void>,
    // 0x0328
    // pub b_clientside: bool,
}

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

CREATE_COMMAND!(
    "listmods",
    ["mods", "lsmods"], 
    "Scans and lists all available and active mods", 
    {},
    false,
    |args| {
        use crate::{resolvers::unchained_integration::run_on_game_thread};
        let mm_lock = || globals().mod_manager.lock().unwrap();

        if mm_lock().as_ref().is_some_and(|mm| mm.get_available().is_empty()) {
            let (tx, rx) = mpsc::channel();
            
            run_on_game_thread(move || {
                if let Some(mm) = mm_lock().as_ref() {
                    sinfo!(f; "Starting scan!");
                    mm.scan_asset_registry();
                    let _ = tx.send(());
                }
                sinfo!(f; "Ended scan!");
            }); 
            
            let _ = rx.recv();
        } else {
            if let Some(mm) = mm_lock().as_ref() {
                let _ = mm.scan_active_mod_actors();
            }
        }

        if let Some(mm) = mm_lock().as_ref() {
            mm.dump_to_console();
        }     
    }
);

pub unsafe fn load_class_from_path(path: &str) -> *mut UClass {
    let wide_path = U16CString::from_str(path).unwrap();
    
    // We pass UClass::StaticClass() as the first argument to ensure 
    // the engine knows we are looking for a Class type.
    // If you don't have StaticClass() bound, passing null_mut() usually works,
    // but the engine will return a UObject* that you must cast.
    let loaded_obj = CALL_ORIGINAL!(StaticLoadObject(
        std::ptr::null_mut(), // Class (optional, null = auto-detect)
        std::ptr::null_mut(), // InOuter
        wide_path.as_ptr(),   // Path
        std::ptr::null(),     // Filename
        0,                    // LoadFlags
        std::ptr::null_mut(), // Sandbox
        false,                // bAllowNativeComponentClass
        std::ptr::null_mut()  // InstancingContext
    ));

    if loaded_obj.is_null() {
        return std::ptr::null_mut();
    }

    // Return the object cast to a UClass
    loaded_obj as *mut UClass
}

pub unsafe fn get_uobject_from_path(path: &str, load_if_missing: bool) -> *mut UObject {
    let wide_path = U16CString::from_str(path).unwrap();
    
    if !load_if_missing {
        // Try to find it in memory first
        return CALL_ORIGINAL!(StaticFindObject(
            std::ptr::null_mut(), 
            std::ptr::null_mut(), 
            wide_path.as_ptr(), 
            false
        ));
    }

    CALL_ORIGINAL_SAFE!(StaticLoadObject(
        std::ptr::null_mut(), // Any class
        std::ptr::null_mut(), // No specific outer
        wide_path.as_ptr(), 
        std::ptr::null(),     // Filename (usually null)
        0,                    // LoadFlags (0 = LoadConfig)
        std::ptr::null_mut(), // Sandbox
        false,                // AllowNativeComponentClass
        std::ptr::null_mut()
    )).expect("Failed to find object")
}

pub unsafe fn get_assets_by_class(cmd: String) -> Result<TArray::<FAssetData>, String> {
    let mut res_arr = TArray::<FAssetData>::default();
    let mut result = TScriptInterface::new();
    let _: *mut TScriptInterface = CALL_ORIGINAL_SAFE!(GetAssetRegistry_Helper(&mut result as *mut TScriptInterface)).expect("GetAssetRegistry_Helper failed");
    
    let mut fname = FName {
        comparison_index: FNameEntryId{ value:0 },
        number: 0,
    };

    let mut fstring2 = FString::from(
        widestring::U16CString::from_str(cmd.as_str())
        .unwrap()
        .as_slice_with_nul());

    let wchar_ptr: *mut u16 = fstring2.as_mut_slice().as_mut_ptr();

    
    let name_res: *mut FName = CALL_ORIGINAL_SAFE!(FNameCtorWchar(&mut fname as *mut FName,
        wchar_ptr,
        crate::ue::EFindName::Find)).expect("FNameCtorWchar failed"); 

    let asset_registry_interface = {
        if result.interface.is_null() {
            // Fallback: If the interface pointer isn't set, 
            // sometimes the Object itself IS the interface.
            crate::serror!("interface is null");
            result.object as *mut c_void
        } else {
            result.interface as *mut c_void
        }
    };

    let out = CALL_ORIGINAL_SAFE!(GetAssetsByClass(
        asset_registry_interface,
        *name_res,
        &mut res_arr as *mut TArray<FAssetData>,
        true
    )).expect("GetAssetsByClass failed");    

    match out {
        true => Ok(res_arr),
        false => Err("".into())
    }
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

            let mod_struct = mobj as *mut ModActorStruct;
            
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
        // TODO: implement
    }

    pub fn destroy_mod_actor(&self, actor_ptr: *mut UObject) {
        // TODO: implement
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
    
    /// Returns a Vec of Mod structs for all currently active mods
    /// Useful for Server Registration
    pub fn get_active_mod_metadata(&self) -> Vec<Mod> {
        let registry = self.registry.lock().unwrap();
        let active = self.active_paths.lock().unwrap();

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