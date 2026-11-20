//! Server-browser request routing, endpoint overrides, and request-token handling.

use crate::chivalry2::engine::FString;
use crate::chivalry2::engine::TSharedPtrIHttpRequest;
use crate::chivalry2::engine::TSharedRefIHttpRequest;
use crate::chivalry2::plugins::online_subsystem_playfab::open_api_merged::open_api_client_api::*;
use crate::chivalry2::plugins::online_subsystem_playfab::playfab_cpp::core::playfab_result_handler::*;
use crate::features::server_browser::urls::playfab_matchmake_url;
use crate::tools::hook_globals::cli_args;
use std::ffi::c_void;

/// # Safety
/// Arguments must satisfy the request's native ABI and game-thread lifetime rules.
#[unchained_macros::bind_func_detour(OpenAPIClientAPI::get_post_get_motd)]
pub unsafe fn get_post_get_motd_detour(
    this: *mut OpenAPIClientAPI,
    result: *mut TSharedPtrIHttpRequest,
    request: *mut GenericRequest,
    delegate: *mut c_void,
) -> *mut TSharedPtrIHttpRequest {
    unsafe {
        with_backend_request(this, request, || {
            this.get_post_get_motd(result, request, delegate)
        })
    }
}

/// # Safety
/// Arguments must satisfy the request's native ABI and game-thread lifetime rules.
#[unchained_macros::bind_func_detour(OpenAPIClientAPI::get_post_get_current_games)]
pub unsafe fn get_post_get_current_games_detour(
    this: *mut OpenAPIClientAPI,
    result: *mut TSharedPtrIHttpRequest,
    request: *mut GenericRequest,
    delegate: *mut c_void,
) -> *mut TSharedPtrIHttpRequest {
    unsafe {
        with_backend_request(this, request, || {
            this.get_post_get_current_games(result, request, delegate)
        })
    }
}

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

pub(crate) unsafe fn with_backend_request(
    this: *mut OpenAPIClientAPI,
    request: *mut GenericRequest,
    call: impl FnOnce() -> *mut TSharedPtrIHttpRequest + std::panic::UnwindSafe,
) -> *mut TSharedPtrIHttpRequest {
    let (this, request) = match unsafe { (this.as_mut(), request.as_mut()) } {
        (Some(this), Some(request)) => (this, request),
        (None, _) => {
            serror!("OpenAPIClientAPI was null");
            return std::ptr::null_mut();
        }
        (_, None) => {
            serror!("Request was null");
            return std::ptr::null_mut();
        }
    };
    let url = crate::features::server_browser::urls::tbio_base_url(
        &crate::tools::hook_globals::cli_args().server_browser_backend,
    );
    let backend_url = FString::from(url.as_str());
    let empty_token = FString::from("");
    let old_url = std::mem::replace(&mut this.url_base, backend_url);
    let old_token = std::mem::replace(&mut request.token, empty_token);
    let result = std::panic::catch_unwind(call);
    this.url_base = old_url;
    request.token = old_token;
    match result {
        Ok(response) => response,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}
