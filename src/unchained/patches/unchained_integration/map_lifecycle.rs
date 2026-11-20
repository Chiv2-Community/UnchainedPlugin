//! Track world state and publish map lifecycle events.

use crate::Chivalry2::TBL::TBLGameInstance::*;
use crate::{
    events::models::{GameEvent, MapChange},
    features::events::EVENT_SYSTEM,
    tools::hook_globals::globals,
    ENGINE_READY,
};
use std::sync::atomic::Ordering;

#[unchained_macros::bind_func_detour(UTBLGameInstance::OnPreLoadMap)]
pub(crate) unsafe fn on_pre_load_map_detour(
    this: *mut UTBLGameInstance,
    url: *mut crate::Chivalry2::Engine::FString,
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
        this.OnPreLoadMap(url);
    }
}

#[unchained_macros::bind_func_detour(UTBLGameInstance::OnPostLoadMap)]
pub(crate) unsafe fn on_post_load_map_detour(
    this: *mut UTBLGameInstance,
    world: *mut crate::Chivalry2::Engine::UWorld,
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
    if let Some(url) = crate::Chivalry2::Engine::UWorld::get_current_map_name(world) {
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
        this.OnPostLoadMap(world);
    }
}
