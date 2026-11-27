//! Native owner: TBL/Plugins/OnlineSubsystemPlayFab/Source/OpenAPIMerged/Public/OpenAPIClientApi.h.
//! Native source unavailable: this header is absent from both checkouts.

use crate::Chivalry2::Engine::FString;
use crate::Chivalry2::Engine::TSharedPtrIHttpRequest;
use binary_bloodhound::{BinaryMatcher, ExactlyOne, HasAddress, Matcher};
use patternsleuth::MemoryTrait;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::LazyLock;
use std::sync::RwLock;

// Request factory used to map endpoint strings to GetPost_* caller entries.
define_custom_resolver!(CreateHttpRequest, {
    OpenAPIClientAPI::request_functions()?.map(|resolved| {
        let (address, offsets) = resolved.value.as_ref();
        sinfo!("Request string → root map:");
        for (name, offset) in offsets {
            sinfo!("  '{}' => 0x{:x}", name, offset);
        }
        *STRING_TO_ROOT_FN
            .write()
            .expect("STRING_TO_ROOT_FN poisoned") = offsets.clone();
        *address
    })
});

// Populate the GetPost_* offset map before request bindings initialize.
unchained_macros_patternsleuth::require_resolver!(CreateHttpRequest);

unchained_macros_patternsleuth::define_offset_resolver!(GetPost_GetMotd);
unchained_macros_patternsleuth::define_offset_resolver!(GetPost_GetCurrentGames);

/// Common native prefix for Get*Post request parameters.
#[unchained_macros_core::runtime_bindings]
#[repr(C)]
#[derive(Debug)]
pub struct GenericRequest {
    _private: [u8; 0xD8],
    pub token: FString,
}

#[unchained_macros_core::runtime_bindings]
#[repr(C)]
#[derive(Debug)]
pub struct OpenAPIClientAPI {
    pub url_base: FString,
}

#[unchained_macros_core::runtime_bindings]
impl OpenAPIClientAPI {
    /// # Safety
    /// Arguments must satisfy the request's native ABI and game-thread lifetime rules.
    #[unchained_macros_core::bind_resolved_func(GetPost_GetMotd)]
    pub unsafe fn GetPost_GetMotd(
        self: *mut Self,
        result: *mut TSharedPtrIHttpRequest,
        request: *mut GenericRequest,
        delegate: *mut c_void,
    ) -> *mut TSharedPtrIHttpRequest {
    }

    /// # Safety
    /// Arguments must satisfy the request's native ABI and game-thread lifetime rules.
    #[unchained_macros_core::bind_resolved_func(GetPost_GetCurrentGames)]
    pub unsafe fn GetPost_GetCurrentGames(
        self: *mut Self,
        result: *mut TSharedPtrIHttpRequest,
        request: *mut GenericRequest,
        delegate: *mut c_void,
    ) -> *mut TSharedPtrIHttpRequest {
    }

    // Other known request keys; no bindings or hooks are installed for these.
    // GetPost_GetPlayerInventory
    // GetPost_LoginPlayFab
    // GetPost_GetCampaigns
    // GetPost_ReAuthServerCustomId
    // GetPost_PostLogin
    // GetPost_Heartbeat
    // GetPost_PreRegisterGame
    // GetPost_GetMyProgress
    // GetPost_GetJoinTicket
    // GetPost_DeregisterGame
    // GetPost_NotifyPlayerJoined
    // GetPost_GrantFlavorEntitlement
    // GetPost_ReportTelemetry
    // GetPost_PhonebookSearchId
    // GetPost_PhonebookGetOwnId
    // GetPost_Award
    // GetPost_RedeemEntitlementEOS
    // GetPost_GrantPendingRewards
    // GetPost_PurchaseWithVirtualCurrency
    // GetPost_GetTitleData
    // GetPost_LoginEOS
    // GetPost_ConfirmJoinTicket
    // GetPost_PhonebookDeleteOwnId

    fn request_functions() -> patternsleuth::resolvers::Result<
        Matcher<'static, (), RequestFunctions, ExactlyOne, impl Send + Sync>,
    > {
        Ok(BinaryMatcher::patterns([
            "40 53 55 56 41 56 48 83 EC 78 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 44 24",
        ])?
        // Preserve the original resolver's first-match selection.
        .first()
        .flat_map(|request| {
            let address = *request.value;
            BinaryMatcher::address(address)
                .callers()
                .root_functions()
                .with_context()
                .map_filter(|(ctx, root)| {
                    let call = root.parent().address();
                    // Target-build LEA: its displacement starts 0x4c past the call.
                    let displacement = call.checked_add(0x49 + 0x3)?;
                    let memory = &ctx.image().memory;
                    let url = memory.read_wstring(memory.rip4(displacement).ok()?).ok()?;
                    let endpoint = url.rsplit('/').next()?;
                    let offset = root.address().checked_sub(ctx.image().base_address)?;
                    Some((format!("GetPost_{endpoint}"), offset))
                })
                // Keep each endpoint paired with its own caller's function entry.
                .transform(move |functions| {
                    let offsets = functions
                        .iter()
                        .map(|node| node.value.as_ref().clone())
                        .collect();
                    (address, offsets)
                })
        })
        .ensure_one())
    }
}

pub static STRING_TO_ROOT_FN: LazyLock<RwLock<HashMap<String, u64>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

inventory::submit! {
    unchained_macros_patternsleuth::OffsetProvider {
        name: "CreateHttpRequest Get*Post hooks",
        map: || STRING_TO_ROOT_FN.read().unwrap().clone(),
    }
}

type RequestFunctions = (u64, HashMap<String, u64>);

// OpenAPIMerged::OpenAPIClientApi::GetCurrentGamesPostResponse.
#[unchained_macros_core::runtime_bindings]
pub type GetCurrentGamesPostResponse = c_void;
