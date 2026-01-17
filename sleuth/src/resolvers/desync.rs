use std::os::raw::c_void;

use crate::{game::engine::ENetMode, sdebug, sinfo, tools::hook_globals::{cli_args, globals}, ue::{FName, UFunction, UObject, UStruct}};
use crate::resolvers::unchained_integration::*;

// Desync patch
// FIXME: Add conditionals to CREATE_HOOK?
define_pattern_resolver!(UNetDriver_GetNetMode, [
    "48 83 EC 28 48 8B 01 ?? ?? ?? ?? ?? ?? 84 C0 ?? ?? 33 C0 38 ?? ?? ?? ?? 02 0F 95 C0 FF C0 48 83 C4",
]);
CREATE_PATCH!(UNetDriver_GetNetMode, BYTES, &[0xB8, 0x01, 0x00, 0x00, 0x00, 0xC3], IF { cli_args().apply_desync_patch });  // mov eax, 1; ret
// CREATE_HOOK!(UNetDriver_GetNetMode, { || cli_args().apply_desync_patch }, NONE, ENetMode, (this_ptr: *mut c_void), {
//     return ENetMode::DEDICATED_SERVER;
//     let mode = CALL_ORIGINAL!(UNetDriver_GetNetMode(this_ptr));
//     match mode {
//         ENetMode::LISTEN_SERVER => ENetMode::DEDICATED_SERVER,
//         _ => mode
//     }
// });

// TODO: is this needed still?
// Blocks SetCameraMode from being executed
define_pattern_resolver!(SetCameraMode, [
    "48 89 5C 24 08 57 48 83 EC 20 48 8B 81 F0 02 00 00 48 8B DA 48 8B F9 48 85 C0 ?? ?? 48 89 90 68 02 00 00",
]);
// void __thiscall APlayerController::SetCameraMode(APlayerController *this,FName param_1)
CREATE_HOOK!(SetCameraMode, { || cli_args().apply_desync_patch }, NONE, (), (this_ptr: *mut c_void, param_1: FName), {
    sinfo!(f; "SetCameraMode: {param_1}");
    return;
});

// Block the ClientSetCameraMode event spam (deadlock)
define_pattern_resolver!(ProcessEvent,["40 55 56 57 41 54 41 55 41 56 41 57 48 81 EC F0 00 00 00 48 8D 6C 24 30 48 89 9D 18 01"]);
CREATE_HOOK!(ProcessEvent, { || false }, NONE, (), (
    object: *mut UObject, 
    function: *mut UFunction, 
    params: *mut c_void
),{
    let func_name = unsafe { (*function).ustruct.ufield.uobject.uobject_base_utility.uobject_base.name_private.to_string() };
    
    if func_name == "ClientSetCameraMode" {
        sdebug!(f; "SetCameramode blocked");
        return;
    }
    else {     
        let mut current_class = unsafe { (*object).uobject_base_utility.uobject_base.class_private as *const UStruct };

        let mut name: String = "".to_string();
        if !current_class.is_null() {
            name = unsafe { (*current_class).ufield.uobject.uobject_base_utility.uobject_base.name_private.to_string() };
        }
        crate::sdebug![f; "\x1b[32m[{}::{}]\x1b[0m ", name, func_name];   
        return CALL_ORIGINAL!(ProcessEvent(object, function, params));    
    }
    // if func_name == "OnPostLoadMap" || func_name == "OnPreLoadMap" {
    let spammy = [
        "ReadyToStartMatch",
        "ReadyToEndMatch",
        "ExecuteUbergraph_MatchmakingStatus",
        "MapToCurve",
    ];
    if !spammy.contains(&func_name.as_str())  {
        let mut current_class = unsafe { (*object).uobject_base_utility.uobject_base.class_private as *const UStruct };

        let mut name: String = "".to_string();
        // 2. Walk up the SuperStruct chain
        if !current_class.is_null() {
            name = unsafe { (*current_class).ufield.uobject.uobject_base_utility.uobject_base.name_private.to_string() };
        }
            crate::sdebug![f; "\x1b[32m[{}::{}]\x1b[0m ", name, func_name];

    }

    CALL_ORIGINAL!(ProcessEvent(object, function, params));
});

// Desync for listen server
// FIXME: This may break map objectives, but fixes(?) desync
define_pattern_resolver!(UGameplay_IsDedicatedServer, [
    "48 83 EC 28 48 85 C9 ?? ?? BA 01 00 00 00 ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 48 8B C8 ?? ?? ?? ?? ?? 83 F8 01 0F 94 C0 48",
]);
CREATE_HOOK!(UGameplay_IsDedicatedServer, { || cli_args().playable_listen }, NONE, bool, (param_1: u64),{
    if let Some(world) = globals().world() {
        let mode = unsafe { o_InternalGetNetMode.call(world) };
        if matches!(mode, ENetMode::DEDICATED_SERVER | ENetMode::LISTEN_SERVER) {
            #[cfg(feature="verbose_hooks")]
            crate::sinfo!(f; "Overriding IsDedicatedServer");
            return true;
        }
    }

    CALL_ORIGINAL!(UGameplay_IsDedicatedServer(param_1))
});