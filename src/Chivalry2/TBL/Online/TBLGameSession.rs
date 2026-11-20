//! Native sources: TBL/Source/TBL/Online/TBLGameSession.h/.cpp (shipping PDB).

// ATBLGameSession::ApproveLogin(const FString&), retained without a binding.
define_pattern_resolver!(ATBLGameSession__ApproveLogin, [
    "48 89 5C 24 18 48 89 74 24 20 55 57 41 54 41 55 41 56 48 8D 6C 24 C9 48 81 EC A0 00 00 00 8B", // EGS
    "48 89 5C 24 10 48 89 74 24 18 55 57 41 54 41 56 41 57 48 8B EC 48 81 EC 80 00 00 00 8B", // STEAM
]);
