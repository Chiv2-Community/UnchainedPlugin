use std::{os::raw::c_void, sync::atomic::{AtomicBool, Ordering}};
use windows::Win32::System::Memory::IsBadReadPtr;

use crate::{ENGINE_READY, WORLD_READY, commands::NATIVE_COMMAND_QUEUE, discord::notifications::MapChangeEvent, dispatch, event, game::engine::ENetMode, tools::hook_globals::{cli_args, globals}, ue::{FName, FString}};


// Sets Server password and rcon flag
define_pattern_resolver!(
    LoadFrontEndMap,
    ["48 8B C4 48 89 50 10 48 89 48 08 55 41 55 48 8D 68 98 48 81 EC 58 01 00 00 83 7A 08 00"]
);
CREATE_HOOK!(LoadFrontEndMap, ACTIVE, NONE, bool, (this_ptr: *mut c_void, param_1: *mut FString), {
    let args = &cli_args();
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
CREATE_HOOK!(InternalGetNetMode, CALLED, ENetMode, (world: *mut c_void), {});

// Disable message on startup
define_pattern_resolver!(EACAntiCheatMesssage, Simple,  [
    "4c 8d 05 ?? ?? ?? ?? 48 8b cf 48 8d ?? ?? ?? ?? ?? fe ff 48 85 db 74 08"
]);
CREATE_PATCH!(EACAntiCheatMesssage, 0xE, NOP, 5);

/*
Run jobs or console commands on the game thread
TODO: Perhaps this should be done in ProcessEvent instead of Engine Tick
*/
use std::sync::Mutex;
type GameThreadJob = Box<dyn FnOnce() + Send>;
static JOB_QUEUE: Mutex<Vec<GameThreadJob>> = Mutex::new(Vec::new());

pub fn run_on_game_thread<F>(f: F)
where
    F: FnOnce() + Send + 'static,
{
    crate::sdebug!(f; "Task added to job queue");
    JOB_QUEUE.lock().unwrap().push(Box::new(f));
}

// static IS_TICKING: AtomicBool = AtomicBool::new(false);
thread_local! {
    static IN_TICK: std::cell::Cell<bool> = std::cell::Cell::new(false);
}
use crate::resolvers::admin_control::o_ExecuteConsoleCommand;
CREATE_HOOK!(UGameEngineTick, ACTIVE, NONE, (), (engine:*mut c_void, delta:f32, state:u8), {

    // FIXME: Just a bunch of bullshit

    CALL_ORIGINAL!(UGameEngineTick(engine, delta, state));
    let reentered = IN_TICK.with(|f| {
        let was = f.get();
        f.set(true);
        was
    });

    if reentered {
        crate::serror!(f; "Re-entrant UGameEngineTick");
        return;
    }
    unsafe {
        if IsBadReadPtr(Some(engine), 8).into() {
            crate::serror!(f; "IsBadReadPtr UGameEngineTick");
            return;
        }

    }
    if !o_UGameEngineTick.is_enabled() {
        crate::serror!(f; "Disabled UGameEngineTick");
        return;
    }

    // Process Native Commands
    // Use try_lock to avoid deadlocking the Game Thread if Discord is stuck
    if let Ok(mut native_cmds) = NATIVE_COMMAND_QUEUE.try_lock() {
        for cmd_str in native_cmds.drain(..) {
            let mut f_cmd = FString::from(cmd_str.as_str());
            unsafe {
                // Wrap in a guard to ensure we don't crash the whole thread
                let _ = std::panic::catch_unwind(move || {
                    CALL_ORIGINAL!(ExecuteConsoleCommand(&mut f_cmd));
                });
            }
        }
    }

    // Process Jobs
    if let Ok(mut queue) = JOB_QUEUE.try_lock() {
        for job in queue.drain(..) {
            let result = std::panic::catch_unwind(
                std::panic::AssertUnwindSafe(|| {
                    job();
                })
            );

            if result.is_err() {
                crate::serror!("GameThreadJob panicked! Catching to prevent engine crash.");
            }
        }
    }

    IN_TICK.with(|f| f.set(false));
});


define_pattern_resolver!(OnPreLoadMap,["48 89 74 24 10 57 48 83 EC 50 83 B9 40 08 00 00 00 48 8D 35"]);
// void __thiscall UTBLGameInstance::OnPreLoadMap(UTBLGameInstance *this,FString *param_1)
CREATE_HOOK!(OnPreLoadMap,(game_instance: *mut c_void, map_url: *mut FString),{
    let original_ptr = map_url;
    let url_w = unsafe { (*map_url).to_string() };
    crate::sinfo![f; "\x1b[32m{}\x1b[0m", url_w];
    
    // TODO: better check for server?
    if globals().world().is_none() && cli_args().is_server() {
        if !ENGINE_READY.load(Ordering::SeqCst) {
            ENGINE_READY.store(true, Ordering::SeqCst);
            log::info!(target: "Engine", "\x1b[32mEngine signaled for initialization\x1b[0m");
        }
    }
    if cli_args().discord_enabled() {
        event!(MapChangeEvent { new_map: url_w });
    }
});

// TODO: looks like this had major changes, needs real signature
// old: "40 55 53 56 57 41 55 41 56 41 57 48 8D AC 24 10 FF FF FF 48 81 EC F0 01 00 00 48 8B 05 07 AE 08 04 48 33 C4 48 89 85 D0 00 00 00 45 33"
define_pattern_resolver!(OnPostLoadMap,["40 55 53 56 57 41 56 41 57 48 8d ac 24 e8 fc ff ff 48 81 ec 18 04 00 00 48 8b 05 89 e7 09 04 48 33 c4 48 89 85 f0 02 00 00 33 c0 48 8b"]);
// void __thiscall UTBLGameInstance::OnPostLoadMap(UTBLGameInstance *this,UWorld *param_1)
CREATE_HOOK!(OnPostLoadMap,(game_instance: *mut c_void, world: *mut c_void),{
    // TODO: Store a single world global, skip the atomic bool+global
    crate::sinfo![f; "\x1b[32mTriggered\x1b[0m"];
    if !WORLD_READY.load(Ordering::SeqCst) {
        WORLD_READY.store(true, Ordering::SeqCst);
        log::info!(target: "World", "\x1b[32mWorld signaled for initialization\x1b[0m");
    }

    if globals().world() != Some(world) {
        globals().set_world(world);
    }
});

// For tracking ingame match progress (Placeholder)
define_pattern_resolver!(SetMatchState,[
    "48 89 5C 24 ?? 56 48 83 EC 20 48 8B DA 48 8B F1 48 39 91 ?? ?? ?? ??"
    ]);
CREATE_HOOK!(SetMatchState,(this_ptr: *mut u8, new_state: FName),{
    log::info![target: "Match", "Match state: \x1b[32m{}\x1b[0m", new_state];
});

// TODO: FViewport unused
// define_pattern_resolver!(FViewport, First, {
//     STEAM: ["48 89 5C 24 08 48 89 74 24 10 48 89 7C 24 18 41 56 48 83 EC 30 33 F6"],
//     EGS: ["48 89 5C 24 08 48 89 6C 24 10 48 89 74 24 18 57 48 83 EC 30 33 ED"],
// });