//! Native sources: TBL/Source/TBL/UI/UserFeedbackAndBugReportsLibrary.h/.cpp (shipping PDB).

define_pattern_resolver!(GetGameInfo, {
    STEAM: ["48 8B C4 48 89 58 ?? 48 89 50 ?? 55 56 57 41 54 41 55 41 56 41 57 48 8D A8 ?? ?? ?? ?? 48 81 EC b0 01 00 00 45 33 FF"],
    OTHER: ["48 8B C4 48 89 58 ?? 48 89 50 ?? 55 56 57 41 54 41 55 41 56 41 57 48 8D A8 ?? ?? ?? ?? 48 81 EC E0 02 00 00 33 FF"],
});

/// Static native helpers only; this is not a UObject instance layout.
pub struct UUserFeedbackAndBugReportsLibrary {
    _private: (),
}

#[unchained_macros::runtime_bindings]
impl UUserFeedbackAndBugReportsLibrary {
    // Static FString return: RCX is caller storage, RDX is world, RAX returns storage.
    #[unchained_macros::bind_resolved_func(GetGameInfo)]
    pub(crate) unsafe fn GetGameInfo(
        storage: *mut crate::Chivalry2::Engine::FString,
        world: *mut crate::Chivalry2::Engine::UWorld,
    ) -> *mut crate::Chivalry2::Engine::FString {
    }
}
