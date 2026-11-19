//! ArgonSDK main: [UTBLGameInstance](https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/TBLGameInstance.h).

use crate::chivalry2::engine::{FLinearColor, FText};
use unchained_macros::bind_resolved_func;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::{
    events::models::{GameEvent, MapChange},
    features::events::EVENT_SYSTEM,
    tools::hook_globals::{cli_args, globals},
    ENGINE_READY,
};

define_pattern_resolver!(LoadFrontEndMap, [
    "48 8B C4 48 89 50 10 48 89 48 08 55 41 55 48 8D 68 98 48 81 EC 58 01 00 00 83 7A 08 00"
]);

define_pattern_resolver!(OnPreLoadMap,["48 89 74 24 10 57 48 83 EC 50 83 B9 40 08 00 00 00 48 8D 35"]);

define_pattern_resolver!(OnPostLoadMap,["40 55 53 56 57 41 56 41 57 48 8d ac 24 e8 fc ff ff 48 81 ec 18 04 00 00 48 8b 05 89 e7 09 04 48 33 c4 48 89 85 f0 02 00 00 33 c0 48 8b"]);

define_pattern_resolver!(TBLGameInstance_AddChatLine, [
    "48 89 6C 24 18 56 41 56 41 57 48 83 EC 40 49 63 E9 4D 8B F8 4C 8B F2 48 8B F1"
]);

mod startup_patch;

/// Opaque native game instance. Only live engine-owned pointers are valid.
#[repr(C)]
pub struct UTBLGameInstance { _opaque: [u8; 0] }

#[unchained_macros::runtime_bindings]
impl UTBLGameInstance {
    #[bind_resolved_func_detour(LoadFrontEndMap)]
    pub(crate) unsafe fn load_front_end_map_detour(self: *mut Self, _url: *mut crate::chivalry2::engine::FString) -> bool {
        static INITIALIZED: AtomicBool = AtomicBool::new(false);
        let args = cli_args();
        let password = args.server_password.as_ref()
            .map(|value| format!("?Password={value}")).unwrap_or_default();
        let rcon = if args.rcon_port.is_some() { "?rcon" } else { "" };
        let first_load = INITIALIZED.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok();
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
        unsafe { self.load_front_end_map(&mut map_url) }
    }
    #[bind_resolved_func_detour(OnPreLoadMap)]
    pub(crate) unsafe fn on_pre_load_map_detour(self: *mut Self, url: *mut crate::chivalry2::engine::FString) {
        crate::features::motd::set_match_active(false);
        let name = unsafe { (*url).copy_to_string().unwrap_or_else(|_| "UnknownMap".to_string()) };
        sinfo!("{}", name);
        globals().set_world(std::ptr::null_mut());
        sdebug!("Reset globals.world to nullptr; world-dependent actions are skipped until post-load");
        if !ENGINE_READY.load(Ordering::SeqCst) {
            ENGINE_READY.store(true, Ordering::SeqCst);
            sinfo!("Engine signaled for initialization");
        }
        unsafe { self.on_pre_load_map(url); }
    }
    #[bind_resolved_func_detour(OnPostLoadMap)]
    pub(crate) unsafe fn on_post_load_map_detour(self: *mut Self, world: *mut crate::chivalry2::engine::UWorld) {
        crate::features::motd::set_match_active(false);
        sdebug!("Triggered");
        match globals().world() {
            Some(current) if current != world => {
                swarn!("World changed from {:p} to {:p} without ever being unset", current, world);
                globals().set_world(world);
            }
            None => {
                sdebug!("globals.world was None before update; setting world from post-load hook");
                globals().set_world(world);
            }
            _ => {}
        }
        if let Some(url) = crate::chivalry2::engine::engine::uworld::get_current_map_name(world) {
            let name = url.split('/').last().unwrap_or("UnknownMapName");
            EVENT_SYSTEM.game_event_publisher.publish(GameEvent::MapChangeEvent(MapChange {
                new_map_url: url.to_string(), new_map_name: name.to_string(),
            }));
        } else {
            swarn!("Current map name unavailable");
        }
        unsafe { self.on_post_load_map(world); }
    }

    /// UTBLGameInstance::AddChatLine, using borrowed native text/color inputs.
    ///
    /// # Safety
    /// Self must be live on the engine thread; text and color pointers must
    /// remain valid for the call. No ownership transfers.
    #[bind_resolved_func(self::TBLGameInstance_AddChatLine)]
    pub unsafe fn add_chat_line(self: *mut Self, sender: *const FText, message: *const FText, channel: u32, want_focus: bool, is_sender_dev: bool, origin_color: *const FLinearColor) {}
}
