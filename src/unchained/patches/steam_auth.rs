//! Redpoint Steam token-check patch; concrete native function/header ownership is unresolved.

// jmp +0x26: skip GetAuthToken and its length check, landing on xor r14b,r14b.
// EBX stays zero from the prologue, so the uninitialized FString is never freed.
CREATE_PATCH!(RedpointSteamSkipTokenCheck, BYTES, &[0xEB, 0x26]);
