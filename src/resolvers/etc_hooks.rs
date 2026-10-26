use std::os::raw::c_void;

use crate::{
    game::engine::{FActorSpawnParameters, FRotator}, resolvers::config_overrides::{LOAD_CONFIG_REENTRY, resolve_profile_override_paths_for_config}, tools::hook_globals::{cli_args, globals, globals_initialized}, ue::{FString, FVector, UClass, UObject},
};

// FString * __thiscall UClass::GetConfigName(UClass *this,FString *__return_storage_ptr__)
// Pass-through hook: the LoadConfig hook calls the original to resolve the base config filename.
// It does not replace or redirect the filename. The real override trigger remains in UObject::LoadConfig.
define_pattern_resolver!(GetConfigName, [
    "48 89 5C 24 10 48 89 74 24 18 57 48 83 EC 70 48 8D B9 E8 00 00 00 48 8B DA 48 8B F1 BA 1F 00 00 00 8B 0F E8 ?? ?? ?? ?? 83 7F 04 00 41 0F 94 C0 44 84 C0 74 15 33 C0 48 89 03 48 8B 3D ?? ?? ?? ?? 48 8B"
]);

CREATE_HOOK!(GetConfigName,  { || cli_args().unchained_profile_location.is_some() }, NONE, *mut FString, (uclass: *mut UClass, ret_ptr: *mut FString), {
    CALL_ORIGINAL!(GetConfigName(uclass, ret_ptr))
});

// void __thiscall UObject::LoadConfig(UObject *this,UClass *param_1,wchar_t *param_2,uint param_3,FProperty *param_4)
// This hook intentionally does not replace the original config path. Instead it first lets the
// engine do the normal load for the requested ini, then re-enters the same LoadConfig call for any
// override file(s) discovered under the unchained-profile root (ini-overrides/maps, map-indices or
// map-profiles). In practice this is a layered load: base config -> normal engine load -> override
// loads. The re-entry guard prevents an infinite recursion loop if an override file triggers the
// same hook again. Depending on the current map/mod context, the effective behavior can look like
// a "triple load": the original config file plus one or more profile overrides.
define_pattern_resolver!(LoadConfig, [
    "40 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 D0 FC FF FF 48 81 EC 30 04 00 00 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 85 10 03 00 00"
]);

CREATE_HOOK!(LoadConfig, { || cli_args().unchained_profile_location.is_some() }, NONE, (), (
    uObject: *mut UObject,
    config_class: *mut UClass,    // UClass* ConfigClass
    filename: *mut u16,            // const TCHAR* Filename
    propagation_flags: u32,        // uint32 PropagationFlags
    property_to_load: *mut c_void  // FProperty* PropertyToLoad
), {
    if LOAD_CONFIG_REENTRY.with(|state| state.get()) || !globals_initialized() {
        return CALL_ORIGINAL!(LoadConfig(
            uObject,
            config_class,
            filename,
            propagation_flags,
            property_to_load
        ));
    }

    let obj = unsafe { &*uObject };
    let actual_class_private = obj.uobject_base_utility.uobject_base.class_private as *mut UClass;

    // Mirror the engine's early return in UObject::LoadConfig: classes without CLASS_Config never
    // reach GetConfigName there, and GetConfigName is fatal for a class whose config name is None.
    // LoadConfig also runs for every non-dynamic CDO, config class or not.
    // https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Private/UObject/Obj.cpp#L2017-L2020
    // https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Private/UObject/Class.cpp#L5244-L5247
    // https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Private/UObject/UObjectGlobals.cpp#L2767-L2770
    const CLASS_CONFIG: u32 = 0x0000_0004;
    let is_config_class = !actual_class_private.is_null()
        && unsafe { (*actual_class_private).class_flags } & CLASS_CONFIG != 0;
    if !is_config_class {
        return CALL_ORIGINAL!(LoadConfig(
            uObject,
            config_class,
            filename,
            propagation_flags,
            property_to_load
        ));
    }

    let lookup_config_name = {
        let mut lookup_storage = FString::default();
        let lookup_ptr = CALL_ORIGINAL!(GetConfigName(actual_class_private, &mut lookup_storage as *mut FString));
        if lookup_ptr.is_null() {
            if lookup_storage.is_empty() {
                None
            } else {
                Some(lookup_storage.to_string())
            }
        } else {
            Some(unsafe { (*lookup_ptr).to_string() })
        }
    };

    let original_filename = if filename.is_null() {
        lookup_config_name
    } else {
        let msg = widestring::U16CStr::from_ptr_str(filename);
        let parsed = msg.to_string_lossy().to_owned();
        if parsed.trim().is_empty() {
            lookup_config_name
        } else {
            Some(parsed)
        }
    };

    let object_path = if uObject.is_null() {
        None
    } else {
        let obj = unsafe { &*uObject };
        let path = obj
            .uobject_base_utility
            .uobject_base
            .get_path_name(None);
        if path.trim().is_empty() {
            None
        } else {
            Some(path)
        }
    };

    let override_filenames = resolve_profile_override_paths_for_config(
        original_filename.as_deref(),
        object_path.as_deref(),
    );

    // The resolver logs a one-line summary per overridden object (config_overrides_verbose_hooks).
    if !override_filenames.is_empty() {
        LOAD_CONFIG_REENTRY.with(|state| state.set(true));

        CALL_ORIGINAL!(LoadConfig(
            uObject,
            config_class,
            filename,
            propagation_flags,
            property_to_load
        ));

        for override_name in &override_filenames {
            let override_path = match widestring::U16CString::from_str(override_name.as_str()) {
                Ok(path) => path,
                Err(_e) => {
                    #[cfg(feature = "config_overrides_verbose_hooks")]
                    crate::warn!("⚠️ Skipping invalid override path {override_name}: {_e}");
                    continue;
                }
            };
            CALL_ORIGINAL!(LoadConfig(
                uObject,
                config_class,
                override_path.as_ptr() as *mut u16,
                propagation_flags,
                property_to_load
            ));
        }

        LOAD_CONFIG_REENTRY.with(|state| state.set(false));
        return;
    }

    CALL_ORIGINAL!(LoadConfig(
        uObject,
        config_class,
        filename,
        propagation_flags,
        property_to_load
    ))
});

// UTBLMapSummary * __cdecl UTBLSystemLibrary::GetMapSummary(UObject *param_1,FString *param_2,bool param_3)
// define_pattern_resolver!(GetMapSummary, [
//     "40 55 53 41 54 48 8D AC 24 60 FE FF FF 48 81 EC A0 02 00 00 48 8B 05 7D 08 FB 03"
// ]);
//
// CREATE_HOOK!(GetMapSummary, ACTIVE, NONE, *mut c_void, (
//     uObject: *mut c_void, fstring: *mut FString, p3: bool
// ), {
//     let url_w = unsafe { (*fstring).to_string() };
//     crate::info!["Map summary: {url_w}"];
//
//     CALL_ORIGINAL!(GetMapSummary(uObject, fstring, p3))
// });

define_pattern_resolver!(GetGameInfo, {
    // "48 8B C4 48 89 58 ?? 48 89 50 ?? 55 56 57 41 54 41 55 41 56 41 57 48 8D A8 ?? ?? ?? ?? 48 81 EC E0 02 00 00", // Universal
    // From sigga
    STEAM: ["48 8B C4 48 89 58 ?? 48 89 50 ?? 55 56 57 41 54 41 55 41 56 41 57 48 8D A8 ?? ?? ?? ?? 48 81 EC b0 01 00 00 45 33 FF"], // STEAM
    OTHER: ["48 8B C4 48 89 58 ?? 48 89 50 ?? 55 56 57 41 54 41 55 41 56 41 57 48 8D A8 ?? ?? ?? ?? 48 81 EC E0 02 00 00 33 FF"], // PDB
});

// FIXME: When is this called?
CREATE_HOOK!(GetGameInfo, ACTIVE, POST,
    *mut FString, (ret_ptr: *mut FString, uWorld: *mut c_void),
    |ret_val: *mut FString| {
        unsafe {
            crate::info!("{}", (&*ret_val).to_string());
        }
        ret_val
    });

define_pattern_resolver!(ClientTravelInternal,["4C 8B DC 48 83 EC 58 33 C0 49 89 5B 08 49 89 6B 10 41 0F B6 E9 49 89 73 18 48"]);
// void __thiscall
// APlayerController::ClientTravelInternal
// 		  (APlayerController *this,FString *param_1,ETravelType param_2,bool param_3,FGuid *param_4)
CREATE_HOOK!(ClientTravelInternal, ACTIVE, c_void, (PC: *mut c_void, arg1: *mut FString, TravelType: u8, arg3: bool, guid: *mut c_void),{

	#[derive(Debug, serde::Deserialize)]
	#[allow(dead_code)]
	struct ConnectionInfo {
		Address: String,
		PartyId: Option<String>,
		PartySize: Option<u32>,
		PlayFabId: Option<String>,
		UnofficialTicket: Option<String>,
	}
	
	//127.0.0.1:7777&PartyId=1231231&PartySize=1&PlayFabId=123123&UnofficialTicket=12323
	fn parse_connection_info(raw: &str) -> Result<ConnectionInfo, Box<dyn std::error::Error>> {
        crate::info!("raw: {}", raw);
		let query_str = raw.replace('?', "&");
		let query_with_address = format!("Address={}", query_str);
		let info: ConnectionInfo = serde_urlencoded::from_str(&query_with_address)?;
		Ok(info)
	}
	
	let string_ref: &FString = unsafe{ &*arg1 };

    let previous_world = globals().world();
    globals().set_world(std::ptr::null_mut());
    match previous_world {
        Some(world_ptr) => crate::warn!(
            "map travel started; invalidated globals.world ({:p}) and skipping world-dependent operations until post-load world is set",
            world_ptr
        ),
        None => crate::debug!(
            "map travel started with globals.world already None; world-dependent operations remain skipped until post-load world is set"
        ),
    }

	match parse_connection_info(string_ref.to_string().as_str()) {
        Ok(info) => {
			crate::info!("info: {:#?}", info);
        }
        Err(e) => crate::error!("Error: {}", e),
    }
});

// AActor * __thiscall UWorld::SpawnActor(UWorld *this,UClass *param_1,FVector *param_2,FRotator *param_3,FActorSpawnParameters *param_4)

define_pattern_resolver!(SpawnActor,["40 53 56 57 48 83 EC 70 48 8B 05 09 4B 63 02 48 33 C4 48 89 44 24 60 0F 28 1D E2 74"]);
CREATE_HOOK!(SpawnActor, INACTIVE, *mut c_void, (world: *mut c_void, class: *mut UClass, position: *mut FVector, rotation: *mut FRotator, spawn_params: *mut FActorSpawnParameters),{
    crate::info!["Triggered!"];
});
/*
ATBLGameMode * __cdecl UTBLSystemLibrary::GetTBLGameMode(UObject *param_1)
*/
define_pattern_resolver![GetTBLGameMode, {
    EGS : ["40 53 48 83 EC 20 48 8B D9 48 85 C9 ?? ?? 48 8B 01 ?? ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 0F 1F 40 00 48 8B 5B 20 48 85 DB ?? ?? 48 8B 03 48 8B CB ?? ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 48 8B 98 28 01 00 00 48 85 DB ?? ?? ?? ?? ?? ?? ?? 48 8B 4B 10 48 83 C0 30 48 63 50 08 3B 51"], // EGS
    OTHER : [
        "40 53 48 83 EC 20 48 8B D9 48 85 C9 74 60 48 8B 01 FF 90 ?? ?? ?? ?? 48 85 C0 75 23 0F 1F 40 00 48 8B 5B 20 48 85 DB 74 11 48 8B 03 48 8B CB FF 90 ?? ?? ?? ?? 48 85 C0 74 E6 48 85 C0 74 2F 48 8B 98 28",
        "40 53 48 83 EC 20 48 8B D9 48 85 C9 ?? ?? 48 8B 01 ?? ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 0F 1F 40 00 48 8B 5B 20 48 85 DB ?? ?? 48 8B 03 48 8B CB ?? ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 48 8B 98 28 01 00 00 48 85 DB ?? ?? ?? ?? ?? ?? ?? 48 8B 4B 10 48 83 C0 30 48 63 50 08 3B 51" // EGS 2.11.4
        ]
}];


CREATE_HOOK!(GetTBLGameMode, INACTIVE, *mut crate::game::chivalry2::ATBLGameMode, (object: *mut c_void),{
    crate::info!["Triggered!"];
});
