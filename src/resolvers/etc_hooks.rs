use std::{cell::Cell, os::raw::c_void};

use crate::{
    game::{
        chivalry2::AActor,
        engine::{FActorSpawnParameters, FRotator, UWorld},
    },
    tools::hook_globals::{cli_args, globals},
    ue::{
        APlayerController, FGuid, FOnlineSessionPlayFab, FString, FVector,
        GetCurrentGamesPostResponse, TArray, TFunctionFIcmpEchoResult, UClass, UObject, TCHAR,
    },
};

// PingSomeServers splits the PlayFab endpoint at the first ':' before it
// reaches FUDPPing::UDPEcho. Scope the Find override to that function and
// return the final ':' for IPv6 endpoints, leaving all other FString::Find
// callers untouched.
thread_local! {
    static IN_PING_SOME_SERVERS: Cell<bool> = const { Cell::new(false) };
    static IN_ON_GET_CURRENT_GAMES: Cell<bool> = const { Cell::new(false) };
}

// Browser path: FindSessions -> OnGetCurrentGames. The endpoint is already
// assembled here, before PingSomeServers starts its own parsing.
define_pattern_resolver!(OnGetCurrentGames_, [
    // "48 89 4C 24 08 55 53 57 41 54 41 55 41 56 48 8D AC 24 58 FE FF FF 48 81 EC A8 02 00 00" // 2.7 PDB
    "48 89 4C 24 08 55 53 56 57 41 54 41 55 41 56 48 8D AC 24 60 FE FF FF"
]);

CREATE_HOOK!(OnGetCurrentGames_, { || !cli_args().is_server() }, NONE, (), (
    this_ptr: *mut FOnlineSessionPlayFab,
    response: *mut GetCurrentGamesPostResponse,
    param_2: i32
), {
    let already_in_endpoint_path = IN_ON_GET_CURRENT_GAMES.with(|state| state.replace(true));
    let result = CALL_ORIGINAL!(OnGetCurrentGames_(this_ptr, response, param_2));
    if !already_in_endpoint_path {
        IN_ON_GET_CURRENT_GAMES.with(|state| state.set(false));
    }
    result
});

define_pattern_resolver!(PingSomeServers, [
    "4C 8B DC 49 89 4B 08 55 56 41 55 49 8D AB 38 FE FF FF 48 81 EC B0 02 00 00 4C 8D A9 40 03 00 00 48 8B F1"
]);

CREATE_HOOK!(PingSomeServers, { || !cli_args().is_server() }, NONE, (), (
    this_ptr: *mut FOnlineSessionPlayFab,
    max_servers: i32
), {
    let already_in_ping = IN_PING_SOME_SERVERS.with(|state| state.replace(true));
    // crate::sinfo!(fn; "PingSomeServers called");
    let result = CALL_ORIGINAL!(PingSomeServers(this_ptr, max_servers));
    if !already_in_ping {
        IN_PING_SOME_SERVERS.with(|state| state.set(false));
    }
    result
});

define_pattern_resolver!(FStringFind, [
    "48 89 5C 24 10 48 89 74 24 18 48 89 7C 24 20 41 54 41 56 41 57 48 83 EC 50 45 8B E1 48 8B FA 4C 8B F1"
]);

CREATE_HOOK!(FStringFind, { || !cli_args().is_server() }, NONE, i32, (
    haystack: *const FString,
    needle: *const TCHAR,
    search_case: i32,
    search_direction: i32,
    start_position: i32
), {
    let in_browser = IN_ON_GET_CURRENT_GAMES.with(|state| state.get());
    let in_ping = IN_PING_SOME_SERVERS.with(|state| state.get());
    let should_fix = (in_browser || in_ping) && !haystack.is_null() && !needle.is_null();

    if !haystack.is_null() && !needle.is_null() {
        let needle_ = unsafe { widestring::U16CStr::from_ptr_str(needle).to_string_lossy() };
        let endpoint_ = unsafe { (*haystack).to_string() };
        // crate::sinfo!(fn; "FStringFind called {endpoint_} : {needle_}");
    }

    if should_fix {
        let needle_text = unsafe { widestring::U16CStr::from_ptr_str(needle) }
            .to_string_lossy();
        if needle_text == ":" {
            let endpoint = unsafe { (*haystack).to_string() };
            if endpoint.matches(':').count() >= 2 {
                if in_browser {
                    // crate::sinfo!(fn; "FString::Find: bare IPv6 endpoint {endpoint}, returning -1");
                    return -1;
                }
                if let Some(last_colon) = endpoint.rfind(':') {
                    let utf16_index = endpoint[..last_colon].encode_utf16().count() as i32;
                    // crate::sinfo!(fn; "FString::Find: IPv6 endpoint {endpoint}, using final colon at {utf16_index}");
                    return utf16_index;
                }
            }
        }
    }

    CALL_ORIGINAL!(FStringFind(
        haystack,
        needle,
        search_case,
        search_direction,
        start_position
    ))
});

// Keep this diagnostic hook so runtime logs show the endpoint that reaches
// the UDP layer. The actual IPv6 correction occurs earlier in OnGetCurrentGames
// and PingSomeServers, not here.
define_pattern_resolver!(UDPEcho, [
    "48 89 5C 24 20 55 56 57 48 81 EC 90 00 00 00 49 8B D8 0F 29 B4 24 80 00 00 00 48 8B E9"
]);

fn bracket_ipv6_udp_endpoint(endpoint: &str) -> Option<String> {
    if endpoint.starts_with('[') || endpoint.matches(':').count() < 2 {
        return None;
    }

    let separator = endpoint.rfind(':')?;
    let host = &endpoint[..separator];
    let port = &endpoint[separator + 1..];
    if host.is_empty() || port.is_empty() || port.parse::<u16>().is_err() {
        return None;
    }

    Some(format!("[{host}]:{port}"))
}

CREATE_HOOK!(UDPEcho, { || !cli_args().is_server() }, NONE, (), (
    endpoint: *mut FString,
    timeout_seconds: f32,
    callback: *mut TFunctionFIcmpEchoResult
), {
    if !endpoint.is_null() {
        let endpoint_text = unsafe { (*endpoint).to_string() };
        // crate::sinfo!(fn; "UDPEcho: received endpoint {endpoint_text}");
        if let Some(normalized) = bracket_ipv6_udp_endpoint(&endpoint_text) {
            // crate::sinfo!(fn; "UDPEcho: normalizing IPv6 endpoint to {normalized}");
            let mut normalized_endpoint = FString::from(normalized.as_str());
            return CALL_ORIGINAL!(UDPEcho(
                &mut normalized_endpoint,
                timeout_seconds,
                callback
            ));
        }
    }
    CALL_ORIGINAL!(UDPEcho(endpoint, timeout_seconds, callback))
});

// UDPEchoImpl splits its target on every ':' and requires exactly two parts.
// Bracketed IPv6 endpoints need a bracket-aware replacement so the ICMP path
// receives [host, port] instead of one element per IPv6 hextet.
// TODO: Nihi: activate this conditionally like above
define_pattern_resolver!(FStringParseIntoArray, [
    "44 88 4C 24 20 53 55 56 57 41 55 41 57 48 83 EC 28 48 8B FA 48 8B D9"
]);

CREATE_HOOK!(FStringParseIntoArray, { || !cli_args().is_server() }, NONE, i32, (
    haystack: *const FString,
    output: *mut TArray<FString>,
    delimiter: *const TCHAR,
    cull_empty: bool
), {
    if !haystack.is_null() && !output.is_null() && !delimiter.is_null() {
        let endpoint = unsafe { (*haystack).to_string() };
        let delimiter_text = unsafe { widestring::U16CStr::from_ptr_str(delimiter) }
            .to_string_lossy();
        if delimiter_text == ":" && endpoint.starts_with('[') {
            if let Some(close) = endpoint.find("]:" ) {
                let host = &endpoint[1..close];
                let port = &endpoint[close + 2..];
                if !host.is_empty() && !port.is_empty() && port.parse::<u16>().is_ok() {
                    let mut parts = TArray::<FString>::new();
                    parts.push(FString::from(host));
                    parts.push(FString::from(port));
                    unsafe { *output = parts };
                    // crate::sinfo!(fn; "ParseIntoArray: IPv6 endpoint split into host and port");
                    return 2;
                }
            }
        }
    }

    CALL_ORIGINAL!(FStringParseIntoArray(haystack, output, delimiter, cull_empty))
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
//     sinfo![fn; "Map summary: {url_w}"];
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
*mut FString, (ret_ptr: *mut FString, uWorld: *mut UWorld),
|ret_val: *mut FString| {
    unsafe {
        sinfo!(fn; "{}", (&*ret_val).to_string());
    }
    ret_val
});

define_pattern_resolver!(ClientTravelInternal,["4C 8B DC 48 83 EC 58 33 C0 49 89 5B 08 49 89 6B 10 41 0F B6 E9 49 89 73 18 48"]);
// void __thiscall
// APlayerController::ClientTravelInternal
// 		  (APlayerController *this,FString *param_1,ETravelType param_2,bool param_3,FGuid *param_4)
CREATE_HOOK!(ClientTravelInternal, ACTIVE, c_void, (PC: *mut APlayerController, arg1: *mut FString, TravelType: u8, arg3: bool, guid: *mut FGuid),{

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
        sinfo!(fn; "raw: {}", raw);
        let query_str = raw.replace('?', "&");
        let query_with_address = format!("Address={}", query_str);
        let info: ConnectionInfo = serde_urlencoded::from_str(&query_with_address)?;
        Ok(info)
    }

    let string_ref: &FString = unsafe{ &*arg1 };

    let previous_world = globals().world();
    globals().set_world(std::ptr::null_mut());
    match previous_world {
        Some(world_ptr) => swarn!(
            fn; "map travel started; invalidated globals.world ({:p}) and skipping world-dependent operations until post-load world is set",
            world_ptr
        ),
        None => sdebug!(
            fn; "map travel started with globals.world already None; world-dependent operations remain skipped until post-load world is set"
        ),
    }

    match parse_connection_info(string_ref.to_string().as_str()) {
        Ok(info) => {
            sinfo!(fn; "info: {:#?}", info);
        }
        Err(e) => serror!(fn; "Error: {}", e),
    }
});

// AActor * __thiscall UWorld::SpawnActor(UWorld *this,UClass *param_1,FVector *param_2,FRotator *param_3,FActorSpawnParameters *param_4)

define_pattern_resolver!(SpawnActor,["40 53 56 57 48 83 EC 70 48 8B 05 09 4B 63 02 48 33 C4 48 89 44 24 60 0F 28 1D E2 74"]);
CREATE_HOOK!(SpawnActor, INACTIVE, *mut AActor, (world: *mut UWorld, class: *mut UClass, position: *mut FVector, rotation: *mut FRotator, spawn_params: *mut FActorSpawnParameters),{
    sinfo![fn; "Triggered!"];
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

CREATE_HOOK!(GetTBLGameMode, INACTIVE, *mut crate::game::chivalry2::ATBLGameMode, (object: *mut UObject),{
    sinfo![fn; "Triggered!"];
});
