//! ArgonSDK main: [PlayFabCpp module](https://github.com/Chiv2-Community/ArgonSDK/tree/main/Plugins/OnlineSubsystemPlayFab/Source/PlayFabCpp).
//! Private request ABI using UE's [IHttpRequest](https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Online/HTTP/Public/Interfaces/IHttpRequest.h).

use crate::chivalry2::engine::FString;
use crate::chivalry2::engine::TSharedRefIHttpRequest;

define_pattern_resolver!(SendRequest, [
    "48 89 5C 24 ?? 48 89 74 24 ?? 48 89 7C 24 ?? 55 41 54 41 55 41 56 41 57 48 8B EC 48 83 EC 40 48 8B D9 49 8B F9"
]);

// The backend patch's exact native owner remains unverified.
define_pattern_resolver!(AllowAlternateBackend, [
    "48 8B C4 55 48 8D 68 ?? 48 81 EC C0 00 00 00 48 89 58 ?? 48 89 70 ?? 48 89 78 ?? 4C 89 60 ?? 4C 89 70"
]);

/// Static request helpers; the return-storage argument is not a receiver.
pub struct PlayFabRequestHandler {
    _private: (),
}

#[unchained_macros::runtime_bindings]
impl PlayFabRequestHandler {
    #[unchained_macros::bind_resolved_func(SendRequest)]
    pub(crate) unsafe fn send_request(
        storage: *mut TSharedRefIHttpRequest,
        url: *mut FString,
        body: *mut FString,
        key: *mut FString,
        value: *mut FString,
    ) -> *mut TSharedRefIHttpRequest {
    }
}

CREATE_PATCH!(AllowAlternateBackend, 0x4D, BYTES, &[0x90, 0xE9]);
