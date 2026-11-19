//! ArgonSDK main: [PlayFabCpp module](https://github.com/Chiv2-Community/ArgonSDK/tree/main/Plugins/OnlineSubsystemPlayFab/Source/PlayFabCpp).
//! The SDK omits PlayFabRequestHandler; its request ABI uses UE's [IHttpRequest](https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Online/HTTP/Public/Interfaces/IHttpRequest.h).

use crate::features::server_browser::urls::playfab_matchmake_url;
use crate::tools::hook_globals::cli_args;
use crate::chivalry2::engine::{FString, TSharedRefIHttpRequest};

define_pattern_resolver!(SendRequest, [
    "48 89 5C 24 ?? 48 89 74 24 ?? 48 89 7C 24 ?? 55 41 54 41 55 41 56 41 57 48 8B EC 48 83 EC 40 48 8B D9 49 8B F9"
]);

mod backend_patch;

// The first parameter is return storage, not a native receiver.
#[unchained_macros::bind_resolved_func_detour(SendRequest)]
pub(crate) unsafe fn send_request_detour(
    storage: *mut TSharedRefIHttpRequest,
    url: *mut FString,
    body: *mut FString,
    key: *mut FString,
    value: *mut FString,
) -> *mut TSharedRefIHttpRequest {
    let url_text = unsafe { (*url).to_string() };
    let (target_url, mut target_auth) = match url_text.as_str() {
        "https://EBF8D.playfabapi.com/Client/Matchmake?sdk=Chiv2_Version" => {
            let target = playfab_matchmake_url(&cli_args().server_browser_backend);
            (FString::from(target.as_str()), Some(FString::from("")))
        }
        "https://EBF8D.playfabapi.com/Match/GetMatchmakingTicket?sdk=Chiv2_Version" => {
            (FString::from("http://localhost"), None)
        }
        _ => return unsafe { send_request(storage, url, body, key, value) },
    };
    let original_url = unsafe { std::mem::replace(&mut *url, target_url) };
    let auth = match target_auth {
        Some(ref mut auth) => auth as *mut FString,
        None => value,
    };
    let result =
        std::panic::catch_unwind(|| unsafe { send_request(storage, url, body, key, auth) });
    unsafe {
        *url = original_url;
    }
    match result {
        Ok(response) => response,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
