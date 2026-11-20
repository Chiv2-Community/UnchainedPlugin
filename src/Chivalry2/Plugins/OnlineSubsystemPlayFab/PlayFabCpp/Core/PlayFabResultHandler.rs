//! Native source: TBL/Plugins/OnlineSubsystemPlayFab/Source/PlayFabCpp/Public/Core/PlayFabResultHandler.h.

use crate::Chivalry2::Engine::FString;
use crate::Chivalry2::Engine::TSharedRefIHttpRequest;

define_pattern_resolver!(PlayFabRequestHandler__SendRequest, [
    "48 89 5C 24 ?? 48 89 74 24 ?? 48 89 7C 24 ?? 55 41 54 41 55 41 56 41 57 48 8B EC 48 83 EC 40 48 8B D9 49 8B F9"
]);

/// Static request helpers; the return-storage argument is not a receiver.
pub struct PlayFabRequestHandler {
    _private: (),
}

#[unchained_macros::runtime_bindings]
impl PlayFabRequestHandler {
    #[unchained_macros::bind_resolved_func(PlayFabRequestHandler__SendRequest)]
    pub(crate) unsafe fn SendRequest(
        storage: *mut TSharedRefIHttpRequest,
        url: *mut FString,
        body: *mut FString,
        key: *mut FString,
        value: *mut FString,
    ) -> *mut TSharedRefIHttpRequest {
    }
}
