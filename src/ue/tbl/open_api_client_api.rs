//! UE 4.25-plus: request handles use [TSharedPtr](https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/Templates/SharedPointer.h).
//! OpenAPIClientApi and its generated request declarations are not shipped in ArgonSDK.

// Overrides for PlayFab (tbio) requests

use crate::ue::FString;
use patternsleuth::{resolvers::unreal::util, MemoryTrait};

/// Generic Class for Get*Post requests
#[repr(C)]
#[derive(Debug)]
pub struct GenericRequest {
    _private: [u8; 0xD8],
    pub token: FString,
}

#[repr(C)]
#[derive(Debug)]
pub struct OpenAPIClientApi {
    pub url_base: FString,
}

/// Generic hook macro for Get*Post methods (e.g. GetMotd, GetCurrentGames)
/// OpenAPIMerged::OpenAPIClientApi::GetMotdPost
//   (OpenAPIClientApi *this,TSharedPtr<> *__return_storage_ptr__,GetMotdPostRequest *param_1,TBaseDelegate<> *param_2)
// Delegate specializations depend on the response type; keep that slot untyped.
macro_rules! CREATE_REQUEST_HOOK {
    ($name:ident) => {
        paste::paste! {
            // These names resolve through CreateHttpRequest's offset registry,
            // not a separate patternsleuth scan.
            #[allow(non_camel_case_types)]
            struct $name;
            #[unchained_macros::runtime_bindings]
            impl OpenAPIClientApi {
            #[bind_resolved_func_detour($name)]
            unsafe fn [<$name _detour>](
                self: *mut Self,
                a2: *mut crate::ue::TSharedPtrIHttpRequest,
                request: *mut GenericRequest,
                a4: *mut std::os::raw::c_void
            ) -> *mut crate::ue::TSharedPtrIHttpRequest {
                let this_ptr = self;
                let (this, req) = match unsafe { (this_ptr.as_mut(), request.as_mut()) } {
                    (Some(t), Some(r)) => (t, r),
                    (None, _) => { $crate::serror!("GCGObj was null"); return std::ptr::null_mut(); }
                    (_, None) => { $crate::serror!("Request was null"); return std::ptr::null_mut(); }
                };
                let url = $crate::features::server_browser::urls::tbio_base_url(
                    &$crate::tools::hook_globals::cli_args().server_browser_backend,
                );
                let backend_url = $crate::ue::FString::from(url.as_str());
                let empty_token = $crate::ue::FString::from("");
                let old_url = std::mem::replace(&mut this.url_base, backend_url);
                let old_token = std::mem::replace(&mut req.token, empty_token);
                let result = std::panic::catch_unwind(|| {
                    unsafe { Self::$name(this_ptr, a2, request, a4) }
                });
                this.url_base = old_url;
                req.token = old_token;
                match result {
                    Ok(response) => response,
                    Err(panic) => std::panic::resume_unwind(panic),
                }
            }
            }
        }
    };
}

#[cfg(feature = "request_dummy_hooks")]
macro_rules! CREATE_REQUEST_HOOK_DUMMY {
    ($name:ident) => {
        paste::paste! {
            #[allow(non_camel_case_types)]
            struct $name;
            #[unchained_macros::runtime_bindings]
            impl OpenAPIClientApi {
            #[bind_resolved_func_detour($name)]
            unsafe fn [<$name _detour>](
                self: *mut Self,
                a2: *mut crate::ue::TSharedPtrIHttpRequest,
                request: *mut GenericRequest,
                a4: *mut std::os::raw::c_void,
            ) -> *mut crate::ue::TSharedPtrIHttpRequest {
                let this_ptr = self;
                if this_ptr.is_null() || request.is_null() {
                    $crate::serror!("Dummy request had null pointers");
                    return std::ptr::null_mut();
                }
                // $crate::sinfo!("{} Dummy: url_base={}", stringify!($name), this.url_base);
                $crate::sinfo!("Handling dummy request");
                unsafe { Self::$name(this_ptr, a2, request, a4) }
            }
            }
        }
    };
}

use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::RwLock;

pub static STRING_TO_ROOT_FN: LazyLock<RwLock<HashMap<String, u64>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

// OpenAPIClientApi::CreateHttpRequest(OpenAPIClientApi *this,TSharedRef<> *__return_storage_ptr__,Request *param_1)
define_pattern_resolver!(
    CreateHttpRequest,
    ["40 53 55 56 41 56 48 83 EC 78 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 44 24",],
    |ctx, patterns| {
        let futures = ::patternsleuth::resolvers::futures::future::join_all(
            patterns
                .iter()
                .map(|p| ctx.scan(::patternsleuth::scanner::Pattern::new(p).unwrap())),
        )
        .await;

        let addr = futures.into_iter().flatten().next().ok_or_else(|| {
            ::patternsleuth::resolvers::ResolveError::new_msg(format!(
                "Failed to find match for CreateHttpRequest with patterns: {patterns:?}",
                patterns = patterns
            ))
        })?;

        let base_addr = *crate::resolvers::BASE_ADDR as u64;
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
            let mut map = STRING_TO_ROOT_FN
                .write()
                .expect("STRING_TO_ROOT_FN poisoned");
            for (s, root) in strings.into_iter().zip(roots.into_iter()) {
                // sinfo!("Request mapping: '{}' => 0x{:x}", s, (root - base_addr) as u64);
                if root >= base_addr {
                    map.insert(s, root - base_addr);
                }
            }
        }

        {
            let map = STRING_TO_ROOT_FN.read().unwrap();
            sinfo!("Request string → root map:");
            for (s, fn_addr) in map.iter() {
                sinfo!("  '{}' => 0x{:x}", s, fn_addr);
            }
        }

        inventory::submit! {
            crate::resolvers::OffsetRegisty {
                name: "CreateHttpRequest Get*Post hooks",
                map: || STRING_TO_ROOT_FN.read().unwrap().clone(),
            }
        }

        Ok(addr)
    }
);

CREATE_REQUEST_HOOK!(GetPost_GetMotd);
CREATE_REQUEST_HOOK!(GetPost_GetCurrentGames);

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
    // GetPost_SetCampaignPurchased,
    GetPost_GetTitleData,
    GetPost_LoginEOS,
    GetPost_ConfirmJoinTicket,
    GetPost_PhonebookDeleteOwnId
);
