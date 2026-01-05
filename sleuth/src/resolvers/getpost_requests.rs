// Overrides for PlayFab (tbio) requests

use crate::{ ue::FString};
use patternsleuth::{MemoryTrait, resolvers::unreal::util};

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
/// OpenAPIMerged::OpenAPIClientApi::GetMotdPost
//   (OpenAPIClientApi *this,TSharedPtr<> *__return_storage_ptr__,GetMotdPostRequest *param_1,TBaseDelegate<> *param_2)
#[macro_export]
macro_rules! CREATE_REQUEST_HOOK {
    ($name:ident) => {
        $crate::CREATE_HOOK!($name, ACTIVE, NONE, *mut std::os::raw::c_void,
            (
                this_ptr: *mut $crate::resolvers::getpost_requests::GenericGCGObj, 
                a2: *mut std::os::raw::c_void, 
                request: *mut $crate::resolvers::getpost_requests::GenericRequest, 
                a4: *mut std::os::raw::c_void
            ), {
                let (this, req) = unsafe {
                    (this_ptr.as_mut().expect("GCGObj was null"),
                     request.as_mut().expect("Request was null"))
                };  
                let old_url = unsafe { std::ptr::read(&this.url_base) };
                let old_token = unsafe { std::ptr::read(&req.token) };
                let backend_url = $crate::backend_url!("/api/tbio");
                let new_fstring = $crate::ue::FString::from("");
                unsafe {
                    std::ptr::write(&mut this.url_base, backend_url);
                    std::ptr::write(&mut req.token, new_fstring);
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
            (
                this_ptr: *mut $crate::resolvers::getpost_requests::GenericGCGObj, 
                a2: *mut std::os::raw::c_void, 
                request: *mut $crate::resolvers::getpost_requests::GenericRequest, 
                a4: *mut std::os::raw::c_void), {
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


use std::collections::HashMap;
use std::sync::RwLock;
use once_cell::sync::Lazy;

pub static STRING_TO_ROOT_FN: Lazy<RwLock<HashMap<String, u64>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

// OpenAPIClientApi::CreateHttpRequest(OpenAPIClientApi *this,TSharedRef<> *__return_storage_ptr__,Request *param_1)
define_pattern_resolver!(CreateHttpRequest, [
    "40 53 55 56 41 56 48 83 EC 78 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 44 24",
], |ctx, patterns| {
    let futures = ::patternsleuth::resolvers::futures::future::join_all(
        patterns.iter().map(|p| ctx.scan(::patternsleuth::scanner::Pattern::new(p).unwrap()))
    ).await;

    let addr = futures.into_iter().flatten().next().ok_or_else(|| {
        ::patternsleuth::resolvers::ResolveError::Msg(
            format!("Failed to find match for CreateHttpRequest with patterns: {patterns:?}", patterns = patterns).into()
        )
    })?;

    let base_addr = crate::globals().get_base_address();
    let mem = &ctx.image().memory;
    let str_offset = 0x49 + 0x3; // offset to LEA call + string addr

    let mut calls = Vec::new();
    let mut strings = Vec::new();

    for call in util::scan_xcalls(ctx, [&addr]).await {
        let str_addr = mem.rip4(call + str_offset).unwrap_or(0);
        if let Ok(s) = mem.read_wstring(str_addr) {
            calls.push(call);
            let suffix = s.rsplit('/').next().map(|s| s.to_string()).unwrap_or(s);
            strings.push(format!("{}_{}", "GetPost", suffix));
        }
    }

    let roots = util::root_functions(ctx, &calls)?;

    {
        let mut map = STRING_TO_ROOT_FN.write().expect("STRING_TO_ROOT_FN poisoned");
        for (s, root) in strings.into_iter().zip(roots.into_iter()) {
            // crate::sinfo!(f; "CreateHttpRequest mapping: '{}' => 0x{:x}", s, (root - base_addr) as u64);
            if root >= base_addr { map.insert(s, (root - base_addr) as u64); }
        }
    }

    {
        let map = STRING_TO_ROOT_FN.read().unwrap();
        crate::sinfo!(f; "CreateHttpRequest string → root map:");
        for (s, fn_addr) in map.iter() {
            crate::sinfo!(f; "  '{}' => 0x{:x}", s, fn_addr);
        }
    }
    
    inventory::submit! {
        crate::resolvers::OffsetRegisty {
            name: "CreateHttpRequest Get*Post hooks",
            map: || STRING_TO_ROOT_FN.read().unwrap().clone(),
        }
    }

    Ok(addr)
});

CREATE_REQUEST_HOOK!(GetPost_GetMotd);
CREATE_REQUEST_HOOK!(GetPost_GetCurrentGames);


#[macro_export]
macro_rules! request_dummy_hooks {
    ($($hook:ident),* $(,)?) => {
        $(
            #[cfg(feature = "request_dummy_hooks")]
            CREATE_REQUEST_HOOK_DUMMY!($hook);
        )*
    }
}

request_dummy_hooks!(
    GetPost_GetPlayerInventory,
    GetPost_LoginPlayFab,
    GetPost_GetCampaigns,
    GetPost_ReAuthServerCustomId,
    GetPost_PostLogin,
    GetPost_Heartbeat,
    GetPost_PreRegisterGame,
    GetPost_GetMyProgress,
    GetPost_GetJoinTicket,
    GetPost_DeregisterGame,
    GetPost_NotifyPlayerJoined,
    GetPost_GrantFlavorEntitlement,
    GetPost_ReportTelemetry,
    GetPost_PhonebookSearchId,
    GetPost_PhonebookGetOwnId,
    GetPost_Award,
    GetPost_RedeemEntitlementEOS,
    GetPost_GrantPendingRewards,
    GetPost_PurchaseWithVirtualCurrency,
    GetPost_SetCampaignPurchased,
    GetPost_GetTitleData,
    GetPost_LoginEOS,
    GetPost_ConfirmJoinTicket,
    GetPost_PhonebookDeleteOwnId
);

