//! Track connection metadata and invalidate world state during travel.

#![allow(c_void_returns)]

use crate::chivalry2::engine::{APlayerController, FGuid, FString};
use crate::tools::hook_globals::globals;
use std::ffi::c_void;

#[unchained_macros::bind_func_detour(APlayerController::ClientTravelInternal)]
pub(crate) unsafe fn client_travel_internal_detour(
    this: *mut APlayerController,
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
    unsafe { this.ClientTravelInternal(url, travel_type, seamless, guid) }
}
