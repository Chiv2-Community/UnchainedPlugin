// Steam shipping executable d82b5eaa122aaee15366499bbce34e325741fecd06d6b7b5490fc650b5afb48b.
// Covers GetLoginStatus, GetAuthToken and the length-only predicate/free.
// Do not broaden without verifying those semantics.
pub(super) const NODE_SIGNATURE: &str = concat!(
    "40 55 53 56 57 41 55 41 56 41 57 48 8D 6C 24 D9 48 81 EC E0 00 00 00 ",
    "4C 8B E9 48 8B FA 33 DB 49 8B F1 89 5D 6F 4D 8B F8 49 8B 09 48 8B 01 ",
    "4C 8B 90 18 01 00 00 48 8B 02 8B 90 70 01 00 00 41 FF D2 83 F8 02 75 2D ",
    "48 8B 0E 48 8D 55 C7 48 8B 01 4C 8B 88 30 01 00 00 48 8B 07 ",
    "44 8B 80 70 01 00 00 41 FF D1 BB 01 00 00 00 39 58 08 7E 05 45 32 F6 EB 03 ",
    "41 B6 01 F6 C3 01 74 0E 48 8B 4D C7 48 85 C9 74 05 E8 ?? ?? ?? ??"
);

// Confirms the SteamUser field, BLoggedOn slot, 1024-byte buffer, session-ticket
// slot and null SteamNetworkingIdentity argument used by this game's Steam API.
pub(super) const TOKEN_SIGNATURE: &str = concat!(
    "40 53 56 57 48 81 EC 80 04 00 00 48 8B 05 ?? ?? ?? ?? 48 33 C4 ",
    "48 89 84 24 50 04 00 00 33 FF 48 89 54 24 48 48 89 3A 48 8B F2 ",
    "48 89 7A 08 48 8B D9 41 83 F8 04 0F 8D ?? ?? ?? ?? ",
    "48 8B 89 88 01 00 00 48 85 C9 0F 84 ?? ?? ?? ?? 48 8B 01 FF 50 08 ",
    "84 C0 0F 84 ?? ?? ?? ?? 48 8B 8B 88 01 00 00 4C 8D 4C 24 40 ",
    "89 7C 24 40 48 8D 54 24 50 41 B8 00 04 00 00 48 89 7C 24 20 ",
    "48 8B 01 FF 50 68"
);
