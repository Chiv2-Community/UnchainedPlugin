// Patch out Redpoint's nonempty-token check and its unused ticket allocation.
// Steam login is still checked by GetLoginStatus(), which calls BLoggedOn().
define_pattern_resolver!(#[allow_missing_platforms(EGS)] RedpointSteamSkipTokenCheck, { STEAM: [
    "40 55 53 56 57 41 55 41 56 41 57 48 8D 6C 24 D9 48 81 EC E0 00 00 00 4C 8B E9 48 8B FA 33 DB 49 8B F1 89 5D 6F 4D 8B F8 49 8B 09 48 8B 01 4C 8B 90 18 01 00 00 48 8B 02 8B 90 70 01 00 00 41 FF D2 83 F8 02 75 2D | 48 8B 0E 48 8D 55 C7 48 8B 01 4C 8B 88 30 01 00 00 48 8B 07 44 8B 80 70 01 00 00 41 FF D1 BB 01 00 00 00 39 58 08 7E 05 45 32 F6 EB 03 41 B6 01 F6 C3 01 74 0E 48 8B 4D C7 48 85 C9 74 05 E8 ?? ?? ?? ??"
] });

// jmp +0x26: skip GetAuthToken and its length check, landing on xor r14b,r14b.
// EBX stays zero from the prologue, so the uninitialized FString is never freed.
CREATE_PATCH!(RedpointSteamSkipTokenCheck, BYTES, &[0xEB, 0x26]);
