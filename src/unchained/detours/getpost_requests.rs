//! Backend endpoint and request-token overrides.

use crate::chivalry2::engine::FString;
use crate::chivalry2::engine::TSharedPtrIHttpRequest;
use crate::chivalry2::plugins::online_subsystem_playfab::open_api_merged::open_api_client_api::*;
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
