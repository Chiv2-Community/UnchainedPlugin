//! UE 4.25-plus: [FString](https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/Containers/UnrealString.h).

use crate::{
    tools::hook_globals::cli_args,
    engine::extensions::fonline_session_playfab::{IN_ON_GET_CURRENT_GAMES, IN_PING_SOME_SERVERS},
    ue::{FString, TArray, TCHAR},
};

define_pattern_resolver!(FStringFind, [
    "48 89 5C 24 10 48 89 74 24 18 48 89 7C 24 20 41 54 41 56 41 57 48 83 EC 50 45 8B E1 48 8B FA 4C 8B F1"
]);
define_pattern_resolver!(
    FStringParseIntoArray,
    ["44 88 4C 24 20 53 55 56 57 41 55 41 57 48 83 EC 28 48 8B FA 48 8B D9"]
);

#[unchained_macros::runtime_bindings]
impl FString {
    #[bind_resolved_func_detour(FStringFind, || !cli_args().is_server())]
    unsafe fn find_detour(
        self: *const Self,
        needle: *const TCHAR,
        search_case: i32,
        search_direction: i32,
        start_position: i32,
    ) -> i32 {
        let in_browser = IN_ON_GET_CURRENT_GAMES.with(|state| state.get());
        let in_ping = IN_PING_SOME_SERVERS.with(|state| state.get());
        if !self.is_null() && !needle.is_null() {
            let needle_text =
                unsafe { widestring::U16CStr::from_ptr_str(needle) }.to_string_lossy();
            let endpoint = unsafe { (*self).to_string() };
            strace!("FStringFind called {endpoint} : {needle_text}");
            if (in_browser || in_ping) && needle_text == ":" && endpoint.matches(':').count() >= 2 {
                if in_browser {
                    strace!("FString::Find: bare IPv6 endpoint {endpoint}, returning -1");
                    return -1;
                }
                if let Some(last_colon) = endpoint.rfind(':') {
                    let index = endpoint[..last_colon].encode_utf16().count() as i32;
                    strace!(
                        "FString::Find: IPv6 endpoint {endpoint}, using final colon at {index}"
                    );
                    return index;
                }
            }
        }
        unsafe { self.find(needle, search_case, search_direction, start_position) }
    }

    #[bind_resolved_func_detour(FStringParseIntoArray, || !cli_args().is_server())]
    unsafe fn parse_into_array_detour(
        self: *const Self,
        output: *mut TArray<FString>,
        delimiter: *const TCHAR,
        cull_empty: bool,
    ) -> i32 {
        if !self.is_null() && !output.is_null() && !delimiter.is_null() {
            let endpoint = unsafe { (*self).to_string() };
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
        unsafe { self.parse_into_array(output, delimiter, cull_empty) }
    }
}
