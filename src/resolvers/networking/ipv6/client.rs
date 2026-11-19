//! IPv6 endpoint parsing in the native server browser and UDP ping client.
use std::cell::Cell;

use crate::{
    tools::hook_globals::cli_args,
    ue::{
        FOnlineSessionPlayFab, FString, GetCurrentGamesPostResponse, TArray,
        TFunctionFIcmpEchoResult, TCHAR,
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
define_pattern_resolver!(
    OnGetCurrentGames_,
    [
        // "48 89 4C 24 08 55 53 57 41 54 41 55 41 56 48 8D AC 24 58 FE FF FF 48 81 EC A8 02 00 00" // 2.7 PDB
        "48 89 4C 24 08 55 53 56 57 41 54 41 55 41 56 48 8D AC 24 60 FE FF FF"
    ]
);

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
    strace!("PingSomeServers called");
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
        strace!("FStringFind called {endpoint_} : {needle_}");
    }

    if should_fix {
        let needle_text = unsafe { widestring::U16CStr::from_ptr_str(needle) }
            .to_string_lossy();
        if needle_text == ":" {
            let endpoint = unsafe { (*haystack).to_string() };
            if endpoint.matches(':').count() >= 2 {
                if in_browser {
                    strace!("FString::Find: bare IPv6 endpoint {endpoint}, returning -1");
                    return -1;
                }
                if let Some(last_colon) = endpoint.rfind(':') {
                    let utf16_index = endpoint[..last_colon].encode_utf16().count() as i32;
                    strace!("FString::Find: IPv6 endpoint {endpoint}, using final colon at {utf16_index}");
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
define_pattern_resolver!(
    UDPEcho,
    ["48 89 5C 24 20 55 56 57 48 81 EC 90 00 00 00 49 8B D8 0F 29 B4 24 80 00 00 00 48 8B E9"]
);

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
        strace!("UDPEcho: received endpoint {endpoint_text}");
        if let Some(normalized) = bracket_ipv6_udp_endpoint(&endpoint_text) {
            strace!("UDPEcho: normalizing IPv6 endpoint to {normalized}");
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
define_pattern_resolver!(
    FStringParseIntoArray,
    ["44 88 4C 24 20 53 55 56 57 41 55 41 57 48 83 EC 28 48 8B FA 48 8B D9"]
);

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
                    strace!("ParseIntoArray: IPv6 endpoint split into host and port");
                    return 2;
                }
            }
        }
    }

    CALL_ORIGINAL!(FStringParseIntoArray(haystack, output, delimiter, cull_empty))
});
