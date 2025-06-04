

define_pattern_resolver!(UTBLLocalPlayer_Exec, [
    // "75 18 ?? ?? ?? ?? 75 12 4d 85 f6 74 0d 41 38 be ?? ?? ?? ?? 74 04 32 db eb 9b 48 8b 5d 7f 49 8b d5 4c 8b 45 77 4c 8b cb 49 8b cf", // EGS - latest
    // "75 17 45 84 ED", // STEAM
    // From Sigga
    "75 ?? 45 84 ed 75 ?? 48 85 f6 74 ?? 40 38 be ?? 01 00 00", // PDB + STEAM
    "75 18 40 38 7d d7 75 12 4d 85 f6 74 0d 41 38 be ?? 01 00 00", // EGS
    // "75 1a 45 84 ed 75 15 48 85 f6 74 10 40 38 be b0 01 00 00 74 07 32 db e9 a6 fd ff ff 48 8b 5d 60 49 8b d6 4c 8b 45 58 4c 8b cb 49 8b cf", // PDB
]);

define_pattern_resolver!(ExecuteConsoleCommand, [
    "40 53 48 83 EC 30 48 8B 05 ?? ?? ?? ?? 48 8B D9 48 8B 90 58 0C 00 00"
]);

// Unused currently
// /// public: void __cdecl FFrame::Step(class UObject *, void *const)
// #[derive(Debug, PartialEq)]
// #[cfg_attr(
//     feature = "serde-resolvers",
//     derive(serde::Serialize, serde::Deserialize)
// )]
// pub struct FText_AsCultureInvariant(pub usize);
// impl_resolver_singleton!(all, FText_AsCultureInvariant, |ctx| async {
//     let patterns = [
//         "48 89 5C 24 18 48 89 74 24 20 41 56 48 83 EC 60 33 C0 48 89 7C 24 78 48 63", // PDB
//         "40 53 55 57 48 83 EC 50 83 7A 08 01 48 8B F9 4C 89 B4 24 80 00 00 00 C7 44 24 70 00 00 00 00 7F 33 E8 ?? ?? ?? ?? 48 8B 58 08 48 8B 08 48 89 4C 24 20 48 89 5C 24 28 48 85 DB 74 04 F0 FF 43 08 8B 40 10 41 BE 01 00 00 00 89 44 24 30 48 8D 44 24 20 EB 18 48 8D 4C 24 38 E8 ?? ?? ?? ?? 48 8B 5C 24 28 41 BE 02 00 00 00 48 8B 08 48 89 0F 48 8B 48 08 48 89 4F 08 48 85 C9 74 04 F0 FF 41 08 8B 40 10 BD FF FF FF FF 89 47 10 41 F6 C6 02 74 46 48 89 74 24 78 41 83 E6 FD 48 8B 74 24 40 48 85 F6 74 2E 8B C5 F0 0F C1 46 08 83 F8 01 75 22 48 8B 06 48 8B CE FF 10 8B C5 F0 0F C1 46 0C 83 F8 01 75 0E 48 8B 06 BA 01 00 00 00 48 8B CE FF 50 ?? 48 8B 74 24 78 41 F6 C6 01 4C 8B B4 24 80 00 00 00 74 2E 48 85 DB 74 29 8B C5 F0 0F C1 43 08 83 F8 01 75 1D 48 8B 03 48 8B CB FF 10 F0 0F C1 6B 0C 83 FD 01 75 0B 48 8B 03 8B D5 48 8B CB FF 50 ?? 83 4F 10 02" // Steam
//     ];

//     let res = join_all(patterns.iter().map(|p| ctx.scan(Pattern::new(p).unwrap()))).await;

//     Ok(FText_AsCultureInvariant(ensure_one(res.into_iter().flatten())?))
// });

define_pattern_resolver!(BroadcastLocalizedChat, [
    "48 89 74 24 10 57 48 83 EC 30 48 8B 01 41 8B F8 48 8B F2 ?? ?? ?? ?? ?? ?? 48 8B C8 48 8D"
]);

define_pattern_resolver!(GetTBLGameMode, [
    "40 53 48 83 EC 20 48 8B D9 48 85 C9 ?? ?? 48 8B 01 ?? ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 0F 1F 40 00 48 8B 5B 20 48 85 DB ?? ?? 48 8B 03 48 8B CB ?? ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 48 8B 98 28 01 00 00 48 85 DB ?? ?? ?? ?? ?? ?? ?? 48 8B 4B 10 48 83 C0 30 48 63 50 08 3B 51", // EGS
    "40 53 48 83 EC 20 48 8B D9 48 85 C9 74 60 48 8B 01 FF 90 ?? ?? ?? ?? 48 85 C0 75 23 0F 1F 40 00 48 8B 5B 20 48 85 DB 74 11 48 8B 03 48 8B CB FF 90 ?? ?? ?? ?? 48 85 C0 74 E6 48 85 C0 74 2F 48 8B 98 28"
]);

define_pattern_resolver!(ClientMessage, [
    "4C 8B DC 48 83 EC 58 33 C0 49 89 5B 08 49 89 73 18 49 8B D8 49 89 43 C8 48 8B F1 49 89 43 D0 49 89 43 D8 49 8D 43"
]);