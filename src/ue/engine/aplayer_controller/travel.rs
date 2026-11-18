//! UE 4.25-plus: [APlayerController](https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/GameFramework/PlayerController.h).

use crate::{
    tools::hook_globals::globals,
    ue::{APlayerController, FGuid, FString},
};
use std::ffi::c_void;

define_pattern_resolver!(
    ClientTravelInternal,
    ["4C 8B DC 48 83 EC 58 33 C0 49 89 5B 08 49 89 6B 10 41 0F B6 E9 49 89 73 18 48"]
);

#[unchained_macros::runtime_bindings]
impl APlayerController {
    #[bind_resolved_func_detour(ClientTravelInternal)]
    unsafe fn client_travel_internal_detour(
        self: *mut Self,
        url: *mut FString,
        travel_type: u8,
        seamless: bool,
        guid: *mut FGuid,
    ) -> c_void {
        #[derive(Debug, serde::Deserialize)]
        #[allow(dead_code)]
        struct ConnectionInfo {
            Address: String,
            PartyId: Option<String>,
            PartySize: Option<u32>,
            PlayFabId: Option<String>,
            UnofficialTicket: Option<String>,
        }
        fn parse_connection_info(raw: &str) -> Result<ConnectionInfo, Box<dyn std::error::Error>> {
            sinfo!("raw: {}", raw);
            let query = raw.replace('?', "&");
            Ok(serde_urlencoded::from_str(&format!("Address={query}"))?)
        }
        let string_ref = unsafe { &*url };
        let previous_world = globals().world();
        globals().set_world(std::ptr::null_mut());
        match previous_world {
            Some(world) => swarn!(
                "map travel started; invalidated globals.world ({:p}) and skipping world-dependent operations until post-load world is set", world
            ),
            None => sdebug!("map travel started with globals.world already None; world-dependent operations remain skipped until post-load world is set"),
        }
        match parse_connection_info(string_ref.to_string().as_str()) {
            Ok(info) => sinfo!("info: {:#?}", info),
            Err(error) => serror!("Error: {}", error),
        }
        unsafe { self.client_travel_internal(url, travel_type, seamless, guid) }
    }
}
