// not working?
define_pattern_resolver!(FViewport, First, {
    STEAM: ["48 89 5C 24 08 48 89 74 24 10 48 89 7C 24 18 41 56 48 83 EC 30 33 F6"],
    EGS: ["48 89 5C 24 08 48 89 6C 24 10 48 89 74 24 18 57 48 83 EC 30 33 ED"],
});

define_pattern_resolver!(
    LoadFrontEndMap,
    ["48 8B C4 48 89 50 10 48 89 48 08 55 41 55 48 8D 68 98 48 81 EC 58 01 00 00 83 7A 08 00"]
);

define_pattern_resolver!(InternalGetNetMode, {
    EGS: ["40 53 48 81 EC 90 00 00 00 48 8B D9 48 8B 49 38 48 85 C9"], // EGS
    STEAM: [
        "40 57 48 81 EC 90 00 00 00 48 8B F9 48 8B",
        "40 53 48 81 EC 90 00 00 00 48 8B D9 48 8B 49 38 48 85 C9" // EGS 2.11.4
        ], // STEAM
});

define_pattern_resolver!(UNetDriver_GetNetMode, [
    "48 83 EC 28 48 8B 01 ?? ?? ?? ?? ?? ?? 84 C0 ?? ?? 33 C0 38 ?? ?? ?? ?? 02 0F 95 C0 FF C0 48 83 C4",
]);

define_pattern_resolver!(UGameplay_IsDedicatedServer, [
    "48 83 EC 28 48 85 C9 ?? ?? BA 01 00 00 00 ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 48 8B C8 ?? ?? ?? ?? ?? 83 F8 01 0F 94 C0 48",
]);

define_pattern_resolver!(EACAntiCheatMesssage, Simple,  [
    "4c 8d 05 ?? ?? ?? ?? 48 8b cf 48 8d ?? ?? ?? ?? ?? fe ff 48 85 db 74 08"
]);
CREATE_PATCH!(EACAntiCheatMesssage, 0xE, NOP, 5);
// CREATE_PATCH_PLATFORM!(STEAM, EACAntiCheatMesssage @ STEAM, 0xF, NOP, 5);
// CREATE_PATCH_PLATFORM!(EGS, EACAntiCheatMesssage @ EGS, 0xE, NOP, 5);