//! Native owner: OnlineSubsystemRedpointEOS/Shared/Authentication/Nodes/LogUserIntoOnlineSubsystemNode.h.
//! Native source unavailable: this header is absent from both checkouts.
//! <https://docs.redpoint.games/docs/auth/write_your_own_authentication_graph/#loguserintoonlinesubsystemnode>

// Steam EXE SHA-256 prefix d82b5eaa122aaee1:
// function RVA 0xeb1a10; pipe-selected site RVA 0xeb1a56.
// Called by FLogUserIntoOnlineSubsystemNode::Execute (RVA 0xeb91f0).
// Its vtable at RVA 0x4514670 also contains GetDebugName (RVA 0xec36e0),
// which returns "FLogUserIntoOnlineSubsystemNode". The helper's method name
// remains unverified; this resolver selects an instruction, not a callable entry.
define_pattern_resolver!(FLogUserIntoOnlineSubsystemNode__GetAuthTokenPatchSite, {
    STEAM: [
    "40 55 53 56 57 41 55 41 56 41 57 48 8D 6C 24 D9 48 81 EC E0 00 00 00 4C 8B E9 48 8B FA 33 DB 49 8B F1 89 5D 6F 4D 8B F8 49 8B 09 48 8B 01 4C 8B 90 18 01 00 00 48 8B 02 8B 90 70 01 00 00 41 FF D2 83 F8 02 75 2D | 48 8B 0E 48 8D 55 C7 48 8B 01 4C 8B 88 30 01 00 00 48 8B 07 44 8B 80 70 01 00 00 41 FF D1 BB 01 00 00 00 39 58 08 7E 05 45 32 F6 EB 03 41 B6 01 F6 C3 01 74 0E 48 8B 4D C7 48 85 C9 74 05 E8 ?? ?? ?? ??"
    ],
    EGS: None,
    XBOX: None,
    OTHER: None,
});
