//! Native source: <https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/UserFeedbackAndBugReportsLibrary.h>.
//! Native owner: TBL/Source/TBL/UI/UserFeedbackAndBugReportsLibrary.h/.cpp.

/// Static native helpers only; this is not a UObject instance layout.
#[unchained_macros_core::runtime_bindings]
pub struct UUserFeedbackAndBugReportsLibrary {
    _private: (),
}

#[unchained_macros_kismet::kismet_runtime_bindings("/Script/TBL.UserFeedbackAndBugReportsLibrary")]
impl UUserFeedbackAndBugReportsLibrary {
    // Static FString return: RCX is caller storage, RDX is world, RAX returns storage.
    pub(crate) unsafe fn GetGameInfo(
        storage: *mut crate::Chivalry2::Engine::FString,
        world: *mut crate::Chivalry2::Engine::UWorld,
    ) -> *mut crate::Chivalry2::Engine::FString {
    }
}
