//! ArgonSDK main: [PlayFabCpp module](https://github.com/Chiv2-Community/ArgonSDK/tree/main/Plugins/OnlineSubsystemPlayFab/Source/PlayFabCpp).
//!
//! Alternate-backend branch patch. The exact native function owner is unverified.
define_pattern_resolver!(AllowAlternateBackend, [
    "48 8B C4 55 48 8D 68 ?? 48 81 EC C0 00 00 00 48 89 58 ?? 48 89 70 ?? 48 89 78 ?? 4C 89 60 ?? 4C 89 70"
]);
CREATE_PATCH!(AllowAlternateBackend, 0x4D, BYTES, &[0x90, 0xE9]);
