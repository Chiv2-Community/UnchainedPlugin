//! IPv6 compatibility for browser endpoint parsing and native UDP pings.

use crate::chivalry2::engine::runtime::online::icmp::icmp::{FUDPPing, TFunctionFIcmpEchoResult};
use crate::chivalry2::engine::{FString, TArray, TCHAR};
use crate::chivalry2::plugins::online_subsystem_playfab::online_session_interface_playfab::*;
use crate::chivalry2::plugins::online_subsystem_playfab::open_api_merged::open_api_client_api::GetCurrentGamesPostResponse;
use crate::tools::hook_globals::cli_args;
use std::cell::Cell;

#[unchained_macros::bind_func_detour(FUDPPing::UDPEcho, || !cli_args().is_server())]
unsafe fn ipv6_udp_echo(
    endpoint: *mut FString,
    timeout: f32,
    callback: *mut TFunctionFIcmpEchoResult,
) {
    if !endpoint.is_null() {
        let text = unsafe { (*endpoint).to_string() };
        strace!("UDPEcho: received endpoint {text}");
        if let Some(normalized) = bracket_ipv6_udp_endpoint(&text) {
            strace!("UDPEcho: normalizing IPv6 endpoint to {normalized}");
            let mut normalized = FString::from(normalized.as_str());
            return unsafe { FUDPPing::UDPEcho(&mut normalized, timeout, callback) };
        }
    }
    unsafe { FUDPPing::UDPEcho(endpoint, timeout, callback) };
}

#[unchained_macros::bind_func_detour(FString::Find, || !cli_args().is_server())]
pub(crate) unsafe fn find_detour(
    this: *const FString,
    needle: *const TCHAR,
    search_case: i32,
    search_direction: i32,
    start_position: i32,
) -> i32 {
    let in_browser = IN_ON_GET_CURRENT_GAMES.with(|state| state.get());
    let in_ping = IN_PING_SOME_SERVERS.with(|state| state.get());
    if !this.is_null() && !needle.is_null() {
        let needle_text = unsafe { widestring::U16CStr::from_ptr_str(needle) }.to_string_lossy();
        let endpoint = unsafe { (*this).to_string() };
        strace!("FStringFind called {endpoint} : {needle_text}");
        if (in_browser || in_ping) && needle_text == ":" && endpoint.matches(':').count() >= 2 {
            if in_browser {
                strace!("FString::Find: bare IPv6 endpoint {endpoint}, returning -1");
                return -1;
            }
            if let Some(last_colon) = endpoint.rfind(':') {
                let index = endpoint[..last_colon].encode_utf16().count() as i32;
                strace!("FString::Find: IPv6 endpoint {endpoint}, using final colon at {index}");
                return index;
            }
        }
    }
    unsafe { this.Find(needle, search_case, search_direction, start_position) }
}

#[unchained_macros::bind_func_detour(FString::ParseIntoArray, || !cli_args().is_server())]
pub(crate) unsafe fn parse_into_array_detour(
    this: *const FString,
    output: *mut TArray<FString>,
    delimiter: *const TCHAR,
    cull_empty: bool,
) -> i32 {
    if !this.is_null() && !output.is_null() && !delimiter.is_null() {
        let endpoint = unsafe { (*this).to_string() };
        let delimiter_text =
            unsafe { widestring::U16CStr::from_ptr_str(delimiter) }.to_string_lossy();
        if delimiter_text == ":" && endpoint.starts_with('[') {
            if let Some(close) = endpoint.find("]:") {
                let host = &endpoint[1..close];
                let port = &endpoint[close + 2..];
                if !host.is_empty() && !port.is_empty() && port.parse::<u16>().is_ok() {
                    let mut parts = TArray::<FString>::new();
                    parts.push(FString::from(host));
                    parts.push(FString::from(port));
                    unsafe {
                        *output = parts;
                    }
                    strace!("ParseIntoArray: IPv6 endpoint split into host and port");
                    return 2;
                }
            }
        }
    }
    unsafe { this.ParseIntoArray(output, delimiter, cull_empty) }
}

#[unchained_macros::bind_func_detour(FOnlineSessionPlayFab::OnGetCurrentGames, || !cli_args().is_server())]
pub(crate) unsafe fn on_get_current_games_detour(
    this: *mut FOnlineSessionPlayFab,
    response: *mut GetCurrentGamesPostResponse,
    extra: i32,
) {
    let already = IN_ON_GET_CURRENT_GAMES.with(|state| state.replace(true));
    unsafe {
        this.OnGetCurrentGames(response, extra);
    }
    if !already {
        IN_ON_GET_CURRENT_GAMES.with(|state| state.set(false));
    }
}

#[unchained_macros::bind_func_detour(FOnlineSessionPlayFab::PingSomeServers, || !cli_args().is_server())]
pub(crate) unsafe fn ping_some_servers_detour(this: *mut FOnlineSessionPlayFab, max_servers: i32) {
    let already = IN_PING_SOME_SERVERS.with(|state| state.replace(true));
    strace!("PingSomeServers called");
    unsafe {
        this.PingSomeServers(max_servers);
    }
    if !already {
        IN_PING_SOME_SERVERS.with(|state| state.set(false));
    }
}

thread_local! {
    pub(crate) static IN_PING_SOME_SERVERS: Cell<bool> = const { Cell::new(false) };
    pub(crate) static IN_ON_GET_CURRENT_GAMES: Cell<bool> = const { Cell::new(false) };
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case::test_case("2001:db8::1:7777", Some("[2001:db8::1]:7777"); "ipv6")]
    #[test_case::test_case("::1:7777", Some("[::1]:7777"); "loopback")]
    #[test_case::test_case("fe80::1%3:7777", Some("[fe80::1%3]:7777"); "scope id")]
    #[test_case::test_case("[::1]:7777", None; "already bracketed")]
    #[test_case::test_case("127.0.0.1:7777", None; "ipv4")]
    #[test_case::test_case("localhost:7777", None; "hostname")]
    #[test_case::test_case("::1:", None; "empty port")]
    #[test_case::test_case("::1:invalid", None; "invalid port")]
    #[test_case::test_case("::1:65536", None; "out of range port")]
    fn ipv6_endpoint_normalization_is_preserved(endpoint: &str, expected: Option<&str>) {
        assert_eq!(bracket_ipv6_udp_endpoint(endpoint).as_deref(), expected);
    }
}
