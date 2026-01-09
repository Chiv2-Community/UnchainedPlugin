use std::{os::raw::c_void, sync::atomic::{AtomicBool, Ordering}};
use crate::{commands::NATIVE_COMMAND_QUEUE, features::discord_bot::ChatMessage, game::engine::ENetMode, sinfo, swarn, tools::hook_globals::globals, ue::FString};
use crate::resolvers::admin_control::o_FText_AsCultureInvariant;
use crate::resolvers::messages::o_BroadcastLocalizedChat;
use crate::resolvers::etc_hooks::o_GetTBLGameMode;

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
    // Old RCON widget
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
CREATE_HOOK!(InternalGetNetMode, INACTIVE, ENetMode, (world: *mut c_void), {
    // if globals().world() != Some(world) {
    //     globals().set_world(world);
    //     #[cfg(feature="verbose_hooks")]
    //     crate::sinfo!(f; "World set to {:?}", world);
    // }
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

use std::sync::Mutex;

type GameThreadJob = Box<dyn FnOnce() + Send>;

static JOB_QUEUE: Mutex<Vec<GameThreadJob>> = Mutex::new(Vec::new());

pub fn run_on_game_thread<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    JOB_QUEUE.lock().unwrap().push(Box::new(f));
}

use once_cell::sync::Lazy;
// A global queue to hold messages waiting to be printed in-game
pub static CHAT_QUEUE: Lazy<Mutex<Vec<ChatMessage>>> = Lazy::new(|| Mutex::new(Vec::new()));

pub fn process_discord_chat_queue() {
    // 1. Get all messages from the queue and clear it
    let mut messages = Vec::new();
    if let Ok(mut queue) = CHAT_QUEUE.lock() {
        if queue.is_empty() { return; }
        messages.append(&mut queue);
    }

    // 2. We are now on the Main Thread, so it's safe to call Unreal
    if let Some(world) = globals().world() {
        use crate::{game::{chivalry2::EChatType, engine::FText}, ue::FString};

        for msg in messages {
            let mut txt = FText::default();
            let mut settings_fstring = FString::from(msg.msg.as_str());

            unsafe {
                // Call Unreal functions safely on the main thread
                let res = TRY_CALL_ORIGINAL!(FText_AsCultureInvariant(&mut txt, &mut settings_fstring)) as *mut FText;
                let game_mode = TRY_CALL_ORIGINAL!(GetTBLGameMode(world));
                
                if !game_mode.is_null() {
                    TRY_CALL_ORIGINAL!(BroadcastLocalizedChat(game_mode, res, msg.chat_type));
                }
            }
            // let game_mode = unsafe { TRY_CALL_ORIGINAL!(GetTBLGameMode(world)) };
            // if !game_mode.is_null() {
            // for chat_type in EChatType::ALL {
            //     // Build the text: "[Admin] (3)"
            //     let debug_str = format!("[{}] ({})", chat_type.as_str(), chat_type as u8);
                
            //     let mut settings_fstring = FString::from(debug_str.as_str());
            //     let mut txt = FText::default();
                
            //     unsafe {
            //         let res = TRY_CALL_ORIGINAL!(FText_AsCultureInvariant(&mut txt, &mut settings_fstring)) as *mut FText;
                    
            //         // Broadcast using the current chat_type to see the color/prefix
            //         TRY_CALL_ORIGINAL!(BroadcastLocalizedChat(game_mode, res, chat_type));
            //     }
            // }
        // }
        }
    }
}

use crate::resolvers::admin_control::o_ExecuteConsoleCommand;
// Executes pending RCON command
// Resolver is handled by patternsleuth
CREATE_HOOK!(UGameEngineTick, (engine:*mut c_void, delta:f32, state:u8), {
    // let mut q = COMMAND_QUEUE.lock().unwrap();
    // while let Some(cmd) = q.pop() {  
    //     if !dispatch_command(cmd.as_str(), true) {
    //         let mut f_cmd = FString::from(cmd.as_str());
    //         CALL_ORIGINAL!(ExecuteConsoleCommand(&mut f_cmd));
    //     }
    // }
    {
        let mut native_cmds = NATIVE_COMMAND_QUEUE.lock().unwrap();
        for cmd_str in native_cmds.drain(..) {
            let mut f_cmd = FString::from(cmd_str.as_str());
            CALL_ORIGINAL!(ExecuteConsoleCommand(&mut f_cmd));
        }
    }

    let mut queue = JOB_QUEUE.lock().unwrap();
    if !queue.is_empty() {
        for job in queue.drain(..) {
            job();
        }
    }

    process_discord_chat_queue();
});

//define_pattern_resolver!(OnPostLoadMap,["40 55 53 56 57 41 55 41 56 41 57 48 8D AC 24 10 FF FF FF 48 81 EC F0 01 00 00 48 8B 05 07 AE 08 04 48 33 C4 48 89 85 D0 00 00 00 45 33"]);
// FIXME: looks like this had major changes, needs real signature
define_pattern_resolver!(OnPostLoadMap,["40 55 53 56 57 41 56 41 57 48 8d ac 24 e8 fc ff ff 48 81 ec 18 04 00 00 48 8b 05 89 e7 09 04 48 33 c4 48 89 85 f0 02 00 00 33 c0 48 8b"]);
// void __thiscall UTBLGameInstance::OnPostLoadMap(UTBLGameInstance *this,UWorld *param_1)
CREATE_HOOK!(OnPostLoadMap,(game_instance: *mut c_void, world: *mut c_void),{
    // crate::sinfo![f;"\x1b[32mTriggered! 0x{:#?}\x1b[0m", world];
    if globals().world() != Some(world) {
        globals().set_world(world);
        #[cfg(feature="verbose_hooks")]
        crate::sinfo!(f; "\x1b[32mWorld set to {:?}\x1b[0m", world);
    }
});


define_pattern_resolver!(OnPreLoadMap,["48 89 74 24 10 57 48 83 EC 50 83 B9 40 08 00 00 00 48 8D 35"]);
// void __thiscall UTBLGameInstance::OnPreLoadMap(UTBLGameInstance *this,FString *param_1)
CREATE_HOOK!(OnPreLoadMap,(game_instance: *mut c_void, map_url: *mut FString),{
    let original_ptr = map_url;
    let url_w = unsafe { (*map_url).to_string() };
    crate::sinfo![f; "\x1b[32mTriggered! {}\x1b[0m", url_w];
    
    // TODO: better check for server?
    if globals().world().is_none() && globals().cli_args.is_server() {
        // Loading into frontend for the first time
        if let Some(mm)  = globals().mod_manager.lock().unwrap().as_ref() {
            // mm.scan_asset_registry();
            mm.update_save_game();
        }
    }
});