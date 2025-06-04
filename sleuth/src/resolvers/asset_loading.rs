

define_pattern_resolver!(FindFileInPakFiles_1, [
    "48 89 5C 24 ?? 48 89 6C 24 ?? 48 89 74 24 ?? 57 41 54 41 55 41 56 41 57 48 83 EC 30 33 FF"
]);

define_pattern_resolver!(FindFileInPakFiles_2, [
    "48 8B C4 4C 89 48 ?? 4C 89 40 ?? 48 89 48 ?? 55 53 48 8B EC"
]);

define_pattern_resolver!(IsNonPakFilenameAllowed, [
    "48 89 5C 24 ?? 48 89 6C 24 ?? 56 57 41 56 48 83 EC 30 48 8B F1 45 33 C0"
]);