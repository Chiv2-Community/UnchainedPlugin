use std::os::raw::c_void;

use crate::ue::FString;

// Console command permissions
define_pattern_resolver![UTBLLocalPlayer_Exec, {
    // "75 18 ?? ?? ?? ?? 75 12 4d 85 f6 74 0d 41 38 be ?? ?? ?? ?? 74 04 32 db eb 9b 48 8b 5d 7f 49 8b d5 4c 8b 45 77 4c 8b cb 49 8b cf", // EGS - latest
    // "75 17 45 84 ED", // STEAM
    // From Sigga
    OTHER: [
        "75 ?? 45 84 ed 75 ?? 48 85 f6 74 ?? 40 38 be ?? 01 00 00",
        "75 18 40 38 7d d7 75 12 4d 85 f6 74 0d 41 38 be ?? 01 00 00"
        ], // PDB + STEAM
    EGS: ["75 18 40 38 7d d7 75 12 4d 85 f6 74 0d 41 38 be ?? 01 00 00"], // EGS
    // "75 1a 45 84 ed 75 15 48 85 f6 74 10 40 38 be b0 01 00 00 74 07 32 db e9 a6 fd ff ff 48 8b 5d 60 49 8b d6 4c 8b 45 58 4c 8b cb 49 8b cf", // PDB
}];
// Allow console command execution
CREATE_PATCH!(UTBLLocalPlayer_Exec, BYTES, &[0xEB]);

// Commands executed by BPs. Also used for rcon and cli
define_pattern_resolver!(
    ExecuteConsoleCommand,
    ["40 53 48 83 EC 30 48 8B 05 ?? ?? ?? ?? 48 8B D9 48 8B 90 58 0C 00 00"]
);
CREATE_HOOK!(ExecuteConsoleCommand, ACTIVE, NONE, c_void, (command: *mut FString), {
    unsafe { sinfo!("Executing: {}", (&*command)); }
    CALL_ORIGINAL!(ExecuteConsoleCommand(command))
});

define_pattern_resolver!(AuthenticateConsoleCommandCaller, [
    "74 ?? 40 32 FF E9 ?? ?? ?? ?? 49 8B 4D"
]);
CREATE_PATCH!(AuthenticateConsoleCommandCaller, BYTES, &[0xEB]);

// FText::AsCultureInvariant(const FString&): select the copying overload.
// The old prologue also matched FromString; the flags-2 epilogue distinguishes
// culture-invariant text from FromString's flags-0x12 result. A new cache key
// prevents reuse of the old resolver's potentially moving-overload address.
define_pattern_resolver!(FText_AsCultureInvariantCopy, [
    "41 83 4E 10 02 4C 8D 5C 24 60 49 8B 5B 20 49 8B C6",
], |ctx, patterns| {
    let matches = ::patternsleuth::resolvers::futures::future::join_all(
        patterns.iter().map(|pattern| ctx.scan(::patternsleuth::scanner::Pattern::new(pattern).unwrap()))
    ).await;
    let addresses: Vec<_> = matches.into_iter().flatten().collect();
    let roots = ::patternsleuth::resolvers::unreal::util::root_functions(ctx, &addresses)?;
    ::patternsleuth::resolvers::ensure_one(roots)
});

// Unused
define_pattern_resolver![GetTBLGameMode, {
    EGS : ["40 53 48 83 EC 20 48 8B D9 48 85 C9 ?? ?? 48 8B 01 ?? ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 0F 1F 40 00 48 8B 5B 20 48 85 DB ?? ?? 48 8B 03 48 8B CB ?? ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 48 8B 98 28 01 00 00 48 85 DB ?? ?? ?? ?? ?? ?? ?? 48 8B 4B 10 48 83 C0 30 48 63 50 08 3B 51"], // EGS
    OTHER : [
        "40 53 48 83 EC 20 48 8B D9 48 85 C9 74 60 48 8B 01 FF 90 ?? ?? ?? ?? 48 85 C0 75 23 0F 1F 40 00 48 8B 5B 20 48 85 DB 74 11 48 8B 03 48 8B CB FF 90 ?? ?? ?? ?? 48 85 C0 74 E6 48 85 C0 74 2F 48 8B 98 28",
        "40 53 48 83 EC 20 48 8B D9 48 85 C9 ?? ?? 48 8B 01 ?? ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 0F 1F 40 00 48 8B 5B 20 48 85 DB ?? ?? 48 8B 03 48 8B CB ?? ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 48 8B 98 28 01 00 00 48 85 DB ?? ?? ?? ?? ?? ?? ?? 48 8B 4B 10 48 83 C0 30 48 63 50 08 3B 51" // EGS 2.11.4
        ]
}];
