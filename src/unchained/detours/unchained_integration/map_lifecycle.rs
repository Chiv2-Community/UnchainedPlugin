//! Initialize mod assets and publish map lifecycle events.

use crate::chivalry2::tbl::tbl_game_instance::*;
use crate::{
    events::models::{GameEvent, MapChange},
    features::events::EVENT_SYSTEM,
    tools::hook_globals::{cli_args, globals},
    ENGINE_READY,
};
use std::sync::atomic::{AtomicBool, Ordering};

#[unchained_macros::bind_func_detour(UTBLGameInstance::load_front_end_map)]
pub(crate) unsafe fn load_front_end_map_detour(
    this: *mut UTBLGameInstance,
    _url: *mut crate::chivalry2::engine::FString,
) -> bool {
    static INITIALIZED: AtomicBool = AtomicBool::new(false);
    let args = cli_args();
    let password = args
        .server_password
        .as_ref()
        .map(|value| format!("?Password={value}"))
        .unwrap_or_default();
    let rcon = if args.rcon_port.is_some() {
        "?rcon"
    } else {
        ""
    };
    let first_load = INITIALIZED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok();
    let startup = if first_load { "?startup" } else { "" };
    if first_load {
        // Mounting precedes frontend loading; registration must not unwind
        // through the native callback.
        let result = std::panic::catch_unwind(|| {
            crate::features::mod_maps::auto_register_all_assets();
        });
        if result.is_err() {
            serror!("Mod asset registration panicked; continuing without it");
        }
    }
    let map_url = format!("Frontend{password}{rcon}{startup}");
    #[cfg(feature = "verbose_hooks")]
    sinfo!("{}", map_url);
    let mut map_url = crate::chivalry2::engine::FString::from(map_url.as_str());
    unsafe { this.load_front_end_map(&mut map_url) }
}

#[unchained_macros::bind_func_detour(UTBLGameInstance::on_pre_load_map)]
pub(crate) unsafe fn on_pre_load_map_detour(
    this: *mut UTBLGameInstance,
    url: *mut crate::chivalry2::engine::FString,
) {
    crate::features::motd::set_match_active(false);
    let name = unsafe {
        (*url)
            .copy_to_string()
            .unwrap_or_else(|_| "UnknownMap".to_string())
    };
    sinfo!("{}", name);
    globals().set_world(std::ptr::null_mut());
    sdebug!("Reset globals.world to nullptr; world-dependent actions are skipped until post-load");
    if !ENGINE_READY.load(Ordering::SeqCst) {
        ENGINE_READY.store(true, Ordering::SeqCst);
        sinfo!("Engine signaled for initialization");
    }
    unsafe {
        this.on_pre_load_map(url);
    }
}

#[unchained_macros::bind_func_detour(UTBLGameInstance::on_post_load_map)]
pub(crate) unsafe fn on_post_load_map_detour(
    this: *mut UTBLGameInstance,
    world: *mut crate::chivalry2::engine::UWorld,
) {
    crate::features::motd::set_match_active(false);
    sdebug!("Triggered");
    match globals().world() {
        Some(current) if current != world => {
            swarn!(
                "World changed from {:p} to {:p} without ever being unset",
                current,
                world
            );
            globals().set_world(world);
        }
        None => {
            sdebug!("globals.world was None before update; setting world from post-load hook");
            globals().set_world(world);
        }
        _ => {}
    }
    if let Some(url) = crate::chivalry2::engine::UWorld::get_current_map_name(world) {
        let name = url.split('/').last().unwrap_or("UnknownMapName");
        EVENT_SYSTEM
            .game_event_publisher
            .publish(GameEvent::MapChangeEvent(MapChange {
                new_map_url: url.to_string(),
                new_map_name: name.to_string(),
            }));
    } else {
        swarn!("Current map name unavailable");
    }
    unsafe {
        this.on_post_load_map(world);
    }
}
