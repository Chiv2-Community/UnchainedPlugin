
// define_pattern_resolver!(FViewport, [
// ]);

// define_pattern_resolver!(LoadFrontEndMap, [
// ]);

define_pattern_resolver!(InternalGetNetMode, [
    "40 53 48 81 EC 90 00 00 00 48 8B D9 48 8B 49 38 48 85 C9", // EGS
    "40 57 48 81 EC 90 00 00 00 48 8B F9 48 8B", // STEAM
]);

define_pattern_resolver!(UNetDriver_GetNetMode, [
    "48 83 EC 28 48 8B 01 ?? ?? ?? ?? ?? ?? 84 C0 ?? ?? 33 C0 38 ?? ?? ?? ?? 02 0F 95 C0 FF C0 48 83 C4",
]);

define_pattern_resolver!(UGameplay_IsDedicatedServer, [
    "48 83 EC 28 48 85 C9 ?? ?? BA 01 00 00 00 ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 48 8B C8 ?? ?? ?? ?? ?? 83 F8 01 0F 94 C0 48",
]);

