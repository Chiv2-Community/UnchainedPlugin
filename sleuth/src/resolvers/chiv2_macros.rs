
/// Returns a backend URL FString given a suffix
#[macro_export]
macro_rules! backend_url {
    ($suffix:expr) => {
        $crate::ue::FString::from(format!(
            "{}{}",
            $crate::globals().cli_args.server_browser_backend
                .as_ref()
                .expect("Missing server_browser_backend"),
            $suffix
        ).as_str())
    };
}

use crate::{ ue::FString};

/// Generic Class for Get*Post requests
#[repr(C)]
#[derive(Debug)]
pub struct GenericRequest {
    _private: [u8; 0xD8],
    pub token: FString
}

#[repr(C)]
#[derive(Debug)]
pub struct GenericGCGObj {
	pub url_base: FString
}


/// Generic hook macro for Get*Post methods (e.g. GetMotd, GetCurrentGames)
#[macro_export]
macro_rules! CREATE_REQUEST_HOOK {
    ($name:ident, $url_suffix:expr) => {
        $crate::CREATE_HOOK!($name, ACTIVE, NONE, *mut std::os::raw::c_void,
            (this_ptr: *mut $crate::resolvers::chiv2_macros::GenericGCGObj, a2: *mut std::os::raw::c_void, request: *mut $crate::resolvers::chiv2_macros::GenericRequest, a4: *mut std::os::raw::c_void), {
                let (this, req) = unsafe {
                    (this_ptr.as_mut().expect("GCGObj was null"),
                     request.as_mut().expect("Request was null"))
                };  
                let old_url = unsafe { std::ptr::read(&this.url_base) };
                let old_token = unsafe { std::ptr::read(&req.token) };
                unsafe {
                    std::ptr::write(&mut this.url_base, $crate::backend_url!($url_suffix));
                    std::ptr::write(&mut req.token, $crate::ue::FString::from(""));
                }
                let result = match std::panic::catch_unwind(|| $crate::CALL_ORIGINAL!($name(this_ptr, a2, request, a4))) {
                    Ok(r) => r,
                    Err(e) => { unsafe { std::ptr::write(&mut this.url_base, old_url); std::ptr::write(&mut req.token, old_token); } std::panic::resume_unwind(e) }
                };
                unsafe { std::ptr::write(&mut this.url_base, old_url); std::ptr::write(&mut req.token, old_token); }
                // crate::sinfo!("{} Hooked: url_base={} token={}, result={:?}", stringify!($name), this.url_base, req.token, result);
                result
            }
        );
    };
}

#[macro_export]
macro_rules! CREATE_REQUEST_HOOK_DUMMY {
    ($name:ident) => {
        $crate::CREATE_HOOK!($name, ACTIVE, NONE, *mut std::os::raw::c_void,
            (this_ptr: *mut $crate::resolvers::chiv2_macros::GenericGCGObj, a2: *mut std::os::raw::c_void, request: *mut $crate::resolvers::chiv2_macros::GenericRequest, a4: *mut std::os::raw::c_void), {
                let (this, req) = unsafe {
                    (this_ptr.as_mut().expect("GCGObj was null"),
                     request.as_mut().expect("Request was null"))
                };  
                // $crate::sinfo!("{} Dummy: url_base={}", stringify!($name), this.url_base);
                $crate::sinfo!(f; "{} Dummy", stringify!($name));
                $crate::CALL_ORIGINAL!($name(this_ptr, a2, request, a4))
            }
        );
    };
}