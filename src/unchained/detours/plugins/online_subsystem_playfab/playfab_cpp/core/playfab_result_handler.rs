//! Plugin replacement for OnlineSubsystemPlayFab/Source/PlayFabCpp/Public/Core/PlayFabResultHandler.h.

use crate::chivalry2::engine::FString;
use crate::chivalry2::engine::TSharedRefIHttpRequest;
use crate::features::server_browser::urls::playfab_matchmake_url;
use crate::tools::hook_globals::cli_args;
use crate::chivalry2::plugins::online_subsystem_playfab::playfab_cpp::core::playfab_result_handler::*;

#[unchained_macros::bind_func_detour(PlayFabRequestHandler::send_request)]
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
        _ => return unsafe { PlayFabRequestHandler::send_request(storage, url, body, key, value) },
    };
    let original_url = unsafe { std::mem::replace(&mut *url, target_url) };
    let auth = match target_auth {
        Some(ref mut auth) => auth as *mut FString,
        None => value,
    };
    let result = std::panic::catch_unwind(|| unsafe {
        PlayFabRequestHandler::send_request(storage, url, body, key, auth)
    });
    unsafe {
        *url = original_url;
    }
    match result {
        Ok(response) => response,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
