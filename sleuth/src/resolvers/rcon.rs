use std::{
    hash::{DefaultHasher, Hash, Hasher}, io::{stdin, BufRead, BufReader}, net::TcpListener, os::raw::c_void, sync::{Arc, Mutex}, thread
};

use log::{error, info, warn};
use once_cell::sync::Lazy;

use crate::ue::{FName, FString, TMap, UClass, UObject};


#[cfg(feature="rcon")]
fn get_rcon_port() -> Option<u16> {
    Some(9001)
}
pub static COMMAND_PENDING: Lazy<Arc<Mutex<Option<bool>>>>   = Lazy::new(|| Arc::new(Mutex::new(None)));
pub static LAST_COMMAND: Lazy<Arc<Mutex<Option<String>>>> = Lazy::new(|| Arc::new(Mutex::new(None)));
// pub static FLAST_COMMAND: Lazy<Arc<Mutex<Option<FString>>>> = Lazy::new(|| Arc::new(Mutex::new(None)));

#[cfg(feature="rcon")]
pub fn handle_rcon() {
    let port = match get_rcon_port() {
        Some(p) => p,
        None => return,
    };

    let listener = TcpListener::bind(("127.0.0.1", port))
        .expect("[RCON] Failed to bind to port");

    info!("[RCON] Listening on 127.0.0.1:{port}");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let cmd_store: Arc<Mutex<Option<String>>> = Arc::clone(&LAST_COMMAND);
                let cmd_pending = Arc::clone(&COMMAND_PENDING);
                thread::spawn(move || {
                    let reader = BufReader::new(stream);
                    for line in reader.lines().map_while(Result::ok) {
                        if !line.trim().is_empty() {
                            warn!("[RCON] Received: {}", line.trim());
                            *cmd_store.lock().unwrap() = Some(line.trim().to_string());
                            *cmd_pending.lock().unwrap() = Some(true);
                        }
                    }
                });
            }
            Err(e) => error!("[RCON] Connection failed: {e}"),
        }
    }
}

#[repr(C)]
pub struct AArgonSDKModBase_C {
    // unknown layout – leave empty or fill in as needed
    _private: [u8; 0], // or a guessed size
}

// pub type TSubclassOf<T> = *mut UClass;
use crate::ue::UEHash;

// impl UEHash for *mut crate::ue::UClass {
//     fn ue_hash(&self) -> u32 {
//         *self as usize as u32
//     }
// }

// impl UEHash for *mut crate::ue::UObject {
//     fn ue_hash(&self) -> u32 {
//         *self as usize as u32
//     }
// }
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
#[derive(Debug)]
pub struct ModActorStruct {
    _private: [u8; 0x30], // or a guessed size
    // pub ModActors: TMap<TSubclassOf<AArgonSDKModBase_C>, FString>, // Offset 0x0030
    pub ModActors: TMap<*mut crate::ue::UClass, FString>, // Offset 0x0030
    pub CustomObjects: TMap<*mut UObject, FString>,                // Offset 0x0080
    // pub CustomObjects: TMap<FName, FName>,                // Offset 0x0080
}
// FIME: Nihi: this need some validation
// maybe a proper prompt etc
#[cfg(feature="cli-commands")]
pub fn handle_cmd() {
    // let line = String::new();

    use std::os::raw::c_void;
    use std::ptr::null_mut;

    use crate::resolvers::asset_registry::{o_Conv_InterfaceToObject, o_FNameCtorWchar, o_GetAsset, o_GetAssetsByClass, FAssetData, GetAssetsByClass_detour_fkt, TScriptInterface};
    use crate::ue::{FName, FNameEntryId, FString, TArray};
    use crate::{resolvers::asset_registry::o_GetAssetRegistry, sinfo};
    use crate::{o_GetAssetRegistry_Helper, swarn};
    loop {
        let mut input = String::new();
        stdin().read_line(&mut input)
            .expect("UTF-8 unsupported");
        let cmd_store: Arc<Mutex<Option<String>>> = Arc::clone(&LAST_COMMAND);
        // if input.as_str() == "findobj" {
        //     crate::sdebug!(f; "findobj {:?}", 123);
        // }

        match input.as_str().trim() {
            "findobj" => {
                sinfo!(f; "Nothing");
            },
            "listmods" => {
                sinfo!(f; "Nothing 2");
                unsafe {
                    // // let mut asdf: *mut c_void = null_mut();
                    // let AR = o_GetAssetRegistry_Helper.call(&mut result as *mut TScriptInterface);
                    // // let test2 = widestring::U16CString::from_str("DA_ModMarker_C");
                    // let cmd = "DA_ModMarker_C".to_string();

                    // let wstr = widestring::U16CString::from_str(cmd.as_str())
                    // .unwrap()
                    // .as_slice_with_nul();
                    // let wstr: *mut u16 = widestring::U16CString::from_str(cmd.as_str())
                    //     .unwrap()
                    //     .as_slice_with_nul()
                    //     .as_ptr() as *mut u16;
                    // let fstring = FString::from(test2);

                    let cmd = "DA_ModMarker_C".to_string();

                    // let mut result = TScriptInterface {
                    //     object: std::ptr::null_mut(),
                    //     interface: std::ptr::null_mut(),
                    // };

                    let mut result = TScriptInterface::new();

                    let AR: *mut TScriptInterface = o_GetAssetRegistry_Helper.call(&mut result as *mut TScriptInterface);
                    // let raw_obj: *mut c_void = o_ConvInterfaceToObject.call(&result);

                    // let AR = o_Conv_InterfaceToObject.call()
                    
                    let mut fname = FName {
                        comparison_index: FNameEntryId{ value:0 },
                        number: 0,
                    };

                    let mut fstring2 = FString::from(
                        widestring::U16CString::from_str(cmd.as_str())
                        .unwrap()
                        .as_slice_with_nul());

                    let wchar_ptr: *mut u16 = fstring2.as_mut_slice().as_mut_ptr();

                    let name_res: *mut FName = o_FNameCtorWchar.call(&mut fname as *mut FName,
                        wchar_ptr,
                        crate::ue::EFindName::Find);
                    // let name_res: *mut FName = o_FNameCtorWchar.call(
                    //     &mut fname as *mut FName,
                    //     wchar_ptr,
                    //     crate::resolvers::asset_registry::EFindName::Add, // instead of Find
                    // );
                    sinfo!(f; "Name found: '{}'", *name_res);     
                    // sinfo!(f; "Name fname dbg: '{:#?}'", fname);    
                    // sinfo!(f; "Name fname: '{}'", fname);      
                    // sinfo!(f; "Name found: '{:#?}'", *name_res);  
                    // sinfo!(f; "FName index: {}, number: {}", (*name_res).comparison_index.value, (*name_res).number);
   
                    
                    let obj = (*AR).object;
                    let obj_with_offset = (obj as *mut u8).wrapping_add(0x28) as *mut UObject; // FIXME: vtable!!!

                    swarn!(f; "AR: {:#?}", AR);
                    // ClassPathName: FName, OutAssetData: * mut TArray<FAssetData>, bSearchSubClasses: bool
                    // let mut res_arr: TArray<FAssetData> = TArray::<FAssetData>::default();
                    // let mut res_arr = TArray::<FAssetData>::with_capacity(64);
                    let mut res_arr = TArray::<FAssetData>::default();
                    // sinfo!(f; "Raw num: {}, max: {}", res_arr.len(), res_arr.capacity());
                    // let out = GetAssetsByClass_detour_fkt(
                    //     obj_with_offset as *mut c_void,
                    //     *name_res,
                    //     &mut res_arr as *mut TArray<FAssetData>,
                    //     true

                    // );
                    // sinfo!(f; "Raw num: {}, max: {}", res_arr.len(), res_arr.capacity());
                    
                    // if (out) {
                    //     sinfo!(f; "Mods: {}", res_arr.len());
                    //     for (cnt, a) in res_arr.as_slice().iter().enumerate() {
                    //         sinfo!(f; "{}: {}", cnt, a.PackagePath);
                    //     }
                    // }
                    let out = o_GetAssetsByClass.call(
                        obj_with_offset as *mut c_void,
                        *name_res,
                        &mut res_arr as *mut TArray<FAssetData>,
                        true
                    );
                    sinfo!(f; "Raw num: {}, max: {}", res_arr.len(), res_arr.capacity());
                    
                    if (out) {
                        sinfo!(f; "Mods: {}", res_arr.len());
                        for (cnt, a) in res_arr.as_mut_slice().iter().enumerate() {
                            sinfo!(f; "{}: {}", cnt, a.PackagePath);
                            let obj: *mut UObject = o_GetAsset.call(a as *const FAssetData as *mut FAssetData); // FIXME: Yeah..
                            if !obj.is_null() {
                                swarn!(f; "Obj: {}", (&*obj).uobject_base_utility.uobject_base.name_private);                                
                                let class_ptr = obj as *mut UClass;
                                let mod_struct = obj as *mut ModActorStruct;
                                // swarn!(f; "Obj: {}", (&*mod_struct).ModActors);       
                                // for (k, v) in (&*mod_struct).ModActors. {

                                // }   
                                // let slice = (&*mod_struct).ModActors.elements.data.as_slice();
                                // let slice = &(&*mod_struct).ModActors.elements;

                                // let asdasda = (&*mod_struct).ModActors.into_hashmap();

                                // let asdasda = (&(*mod_struct).ModActors).to_hashmap();
                                for (a, b) in  (&*mod_struct).ModActors.iter() {
                                    let mut name = FName::default();
                                    if !a.is_null() {
                                        let test = *a;
                                        name = (*test).ustruct.ufield.uobject.uobject_base_utility.uobject_base.name_private;
                                    }
                                    swarn!(f; "\tName: {}", name);   
                                    swarn!(f; "\tDescription: {}", b);   
                                }



                                // let class = unsafe { (*obj).get_class() }; // Or use the class field
                                // let name = unsafe { &(*(*obj)).name_private };
                                // swarn!(f; "Obj: {}", name);
                            }
                        }
                    }
                }
            }
            _ => {     
                sinfo!(f; "Unmatched cmd: '{}'", input.as_str());        
                *cmd_store.lock().unwrap() = Some(input.trim().to_string());
            }

        }
    }
}