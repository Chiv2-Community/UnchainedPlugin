

define_pattern_resolver!(ATBLPlayerController__GetOwnershipFromPlayerControllerAndState, [
    "40 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 B0 FD", // EGS
    "40 55 56 41 54 41 55 41 56 41 57 48 8D AC 24 B8", // STEAM
    "40 55 53 56 57 41 54 41 55 41 56 41 57 48 8d ac 24 38 fd"// PDB
]);

define_pattern_resolver!(ATBLPlayerController__CanUseLoadoutItem, [
    "48 89 5C 24 08 48 89 74 24 10 55 57 41 55 41 56 41 57 48 8B EC 48 81 EC 80 00 00", // EGS
    // "48 89 5C 24 08 48 89 74 24 18 55 57 41 55 41 56 41 57 48 8B EC 48 83 EC", // STEAM
    // from sigga
    // "48 89 5C 24 08 48 89 74 24 10 55 57 41 55 41 56 41 57 48 8B EC 48 81 EC 80 00 00", // EGS
    "48 89 5C 24 08 48 89 74 24 18 55 57 41 55 41 56 41 57 48 8B EC 48 83 EC 60 49 8B 31 33 FF C6 02 00", // STEAM
]);

define_pattern_resolver!(ATBLPlayerController__CanUseCharacter, [
    "48 89 5C 24 08 48 89 6C 24 10 48 89 74 24 18 48 89 7C 24 20 41 56 48 83 EC 50 49 8B 18", // universal
]);

define_pattern_resolver!(ATBLPlayerController__ConditionalInitializeCustomizationOnServer, [
    "48 89 54 24 10 53 56 57 41 54 48 83 EC 78 48 8B 99 60 02 00 00 48 8B F2 0F B6", // EGS
    "48 89 54 24 10 53 55 57 41 54 48 83 EC 78", // STEAM
    // From Sigga
    // Did the function change?
    "41 54 48 81 EC 80 00 00 00 80 B9 F8 00 00 00 03 4C 8B E1 ?? ?? ?? ?? ?? ?? 80 B9 20 13 00 00 00 ?? ?? ?? ?? ?? ?? 80 B9 21", // PDB
]);