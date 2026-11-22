//! Server-browser request routing, endpoint overrides, and request-token handling.

use crate::Chivalry2::Engine::FString;
use crate::Chivalry2::Engine::TSharedPtrIHttpRequest;
use crate::Chivalry2::Engine::TSharedRefIHttpRequest;
use crate::Chivalry2::Plugins::OnlineSubsystemPlayFab::OpenAPIMerged::OpenAPIClientApi::*;
use crate::Chivalry2::Plugins::OnlineSubsystemPlayFab::PlayFabCpp::Core::PlayFabResultHandler::*;
use crate::features::server_browser::urls::playfab_matchmake_url;
use crate::tools::hook_globals::cli_args;
use std::ffi::c_void;

/// # Safety
/// Arguments must satisfy the request's native ABI and game-thread lifetime rules.
#[unchained_macros_core::bind_func_detour(OpenAPIClientAPI::GetPost_GetMotd)]
pub unsafe fn get_post_get_motd_detour(
    this: *mut OpenAPIClientAPI,
    result: *mut TSharedPtrIHttpRequest,
    request: *mut GenericRequest,
    delegate: *mut c_void,
) -> *mut TSharedPtrIHttpRequest {
    unsafe {
        with_backend_request(this, request, || {
            this.GetPost_GetMotd(result, request, delegate)
        })
    }
}

/// # Safety
/// Arguments must satisfy the request's native ABI and game-thread lifetime rules.
#[unchained_macros_core::bind_func_detour(OpenAPIClientAPI::GetPost_GetCurrentGames)]
pub unsafe fn get_post_get_current_games_detour(
    this: *mut OpenAPIClientAPI,
    result: *mut TSharedPtrIHttpRequest,
    request: *mut GenericRequest,
    delegate: *mut c_void,
) -> *mut TSharedPtrIHttpRequest {
    unsafe {
        with_backend_request(this, request, || {
            this.GetPost_GetCurrentGames(result, request, delegate)
        })
    }
}

#[unchained_macros_core::bind_func_detour(PlayFabRequestHandler::SendRequest)]
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
        _ => return unsafe { PlayFabRequestHandler::SendRequest(storage, url, body, key, value) },
    };
    let original_url = unsafe { std::mem::replace(&mut *url, target_url) };
    let auth = match target_auth {
        Some(ref mut auth) => auth as *mut FString,
        None => value,
    };
    let result = std::panic::catch_unwind(|| unsafe {
        PlayFabRequestHandler::SendRequest(storage, url, body, key, auth)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_request_pointers_do_not_call_native_code_or_prepare_overrides() {
        let client = std::ptr::null_mut::<OpenAPIClientAPI>();
        let request = std::ptr::null_mut::<GenericRequest>();
        let result =
            unsafe { with_backend_request(client, request, || panic!("must not call native code")) };
        assert!(result.is_null());
    }
}
