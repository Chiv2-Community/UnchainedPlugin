use std::{os::raw::c_void, sync::atomic::{AtomicBool, Ordering}};
use crate::{features::commands::COMMAND_QUEUE, game::engine::ENetMode, tools::hook_globals::globals, ue::FString};

// not working?
define_pattern_resolver!(FViewport, First, {
    STEAM: ["48 89 5C 24 08 48 89 74 24 10 48 89 7C 24 18 41 56 48 83 EC 30 33 F6"],
    EGS: ["48 89 5C 24 08 48 89 6C 24 10 48 89 74 24 18 57 48 83 EC 30 33 ED"],
});

// Sets Server password and rcon flag
define_pattern_resolver!(
    LoadFrontEndMap,
    ["48 8B C4 48 89 50 10 48 89 48 08 55 41 55 48 8D 68 98 48 81 EC 58 01 00 00 83 7A 08 00"]
);
CREATE_HOOK!(LoadFrontEndMap, ACTIVE, NONE, bool, (this_ptr: *mut c_void, param_1: *mut FString), {
    let args = &globals().cli_args;
    static INITIALIZED: AtomicBool = AtomicBool::new(false);
    let pwd_opt = args.server_password.as_ref()
        .map(|p| format!("?Password={}", p))
        .unwrap_or_default();
    let rcon_opt = if args.rcon_port.is_some() {"?rcon"} else {""};
    let init_opt = match INITIALIZED.load(Ordering::Relaxed) {
        true => "",
        false => {
            INITIALIZED.store(true, Ordering::Relaxed);
            "?startup"
        }
    };

    let map_url = format!("Frontend{pwd_opt}{rcon_opt}{init_opt}");
    #[cfg(feature="verbose_hooks")]
    crate::sinfo!(f; "{}", map_url);

    let mut map_url_fs = FString::from(map_url.as_str());
    CALL_ORIGINAL!(LoadFrontEndMap(this_ptr, &mut map_url_fs))
});

// Used to set World reference
define_pattern_resolver!(InternalGetNetMode, {
    EGS: ["40 53 48 81 EC 90 00 00 00 48 8B D9 48 8B 49 38 48 85 C9"], // EGS
    STEAM: [
        "40 57 48 81 EC 90 00 00 00 48 8B F9 48 8B",
        "40 53 48 81 EC 90 00 00 00 48 8B D9 48 8B 49 38 48 85 C9" // EGS 2.11.4
        ], // STEAM
});
CREATE_HOOK!(InternalGetNetMode, ACTIVE, ENetMode, (world: *mut c_void), {
    if globals().world() != Some(world) {
        globals().set_world(world);
        #[cfg(feature="verbose_hooks")]
        crate::sinfo!(f; "World set to {:?}", world);
    }
});

// Desync patch
// FIXME: Add conditionals to CREATE_HOOK?
define_pattern_resolver!(UNetDriver_GetNetMode, [
    "48 83 EC 28 48 8B 01 ?? ?? ?? ?? ?? ?? 84 C0 ?? ?? 33 C0 38 ?? ?? ?? ?? 02 0F 95 C0 FF C0 48 83 C4",
]);
CREATE_HOOK!(UNetDriver_GetNetMode, ACTIVE, NONE, ENetMode, (this_ptr: *mut c_void), {
    if !globals().cli_args.apply_desync_patch {
        return CALL_ORIGINAL!(UNetDriver_GetNetMode(this_ptr));
    }
    #[cfg(feature="verbose_hooks")]
    crate::sinfo!(f; "Overriding UNetDriver_GetNetMode");
    let mode = CALL_ORIGINAL!(UNetDriver_GetNetMode(this_ptr));
    match mode {
        ENetMode::LISTEN_SERVER => ENetMode::DEDICATED_SERVER,
        _ => mode
    }
});

// FIXME: This may break map objectives, but fixes(?) desync
define_pattern_resolver!(UGameplay_IsDedicatedServer, [
    "48 83 EC 28 48 85 C9 ?? ?? BA 01 00 00 00 ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 48 8B C8 ?? ?? ?? ?? ?? 83 F8 01 0F 94 C0 48",
]);
CREATE_HOOK!(UGameplay_IsDedicatedServer, ACTIVE, NONE, bool, (param_1: u64),{
    if globals().cli_args.playable_listen {
        if let Some(world) = globals().world() {
            let mode = unsafe { o_InternalGetNetMode.call(world) };
            if matches!(mode, ENetMode::DEDICATED_SERVER | ENetMode::LISTEN_SERVER) {
                #[cfg(feature="verbose_hooks")]
                crate::sinfo!(f; "Overriding IsDedicatedServer");
                return true;
            }
        }
    }

    CALL_ORIGINAL!(UGameplay_IsDedicatedServer(param_1))
});

define_pattern_resolver!(EACAntiCheatMesssage, Simple,  [
    "4c 8d 05 ?? ?? ?? ?? 48 8b cf 48 8d ?? ?? ?? ?? ?? fe ff 48 85 db 74 08"
]);
CREATE_PATCH!(EACAntiCheatMesssage, 0xE, NOP, 5);
// CREATE_PATCH_PLATFORM!(STEAM, EACAntiCheatMesssage @ STEAM, 0xF, NOP, 5);
// CREATE_PATCH_PLATFORM!(EGS, EACAntiCheatMesssage @ EGS, 0xE, NOP, 5);

use crate::resolvers::admin_control::o_ExecuteConsoleCommand;
// Executes pending RCON command
// Resolver is handled by patternsleuth
CREATE_HOOK!(UGameEngineTick, (engine:*mut c_void, delta:f32, state:u8), {
    let mut q = COMMAND_QUEUE.lock().unwrap();
    while let Some(cmd) = q.pop() {
        log::info!(target: "Commands", "Console command: {cmd}");
        CALL_ORIGINAL!(ExecuteConsoleCommand(&mut FString::from(cmd.as_str())));
    }
});
