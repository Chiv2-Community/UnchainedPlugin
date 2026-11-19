//! Plugin replacements for `crate::chivalry2::engine::core::containers::unreal_string`.

use crate::chivalry2::engine::core::containers::unreal_string::*;
use crate::chivalry2::engine::TArray;
use crate::chivalry2::engine::TCHAR;
use crate::tools::hook_globals::cli_args;
use crate::unchained::detours::integrations::fonline_session_playfab::{
    IN_ON_GET_CURRENT_GAMES, IN_PING_SOME_SERVERS,
};

#[unchained_macros::bind_func_detour(FString::find, || !cli_args().is_server())]
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
    unsafe { this.find(needle, search_case, search_direction, start_position) }
}

#[unchained_macros::bind_func_detour(FString::parse_into_array, || !cli_args().is_server())]
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
    unsafe { this.parse_into_array(output, delimiter, cull_empty) }
}
