//! Native source: TBL/Plugins/OnlineSubsystemPlayFab/Source/PlayFabCpp/Public/Core/PlayFabResultHandler.h.

use crate::chivalry2::engine::FString;
use crate::chivalry2::engine::TSharedRefIHttpRequest;

define_pattern_resolver!(SendRequest, [
    "48 89 5C 24 ?? 48 89 74 24 ?? 48 89 7C 24 ?? 55 41 54 41 55 41 56 41 57 48 8B EC 48 83 EC 40 48 8B D9 49 8B F9"
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
