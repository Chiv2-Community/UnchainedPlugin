use futures::future::join_all;
use patternsleuth::{resolvers::{ensure_one, impl_resolver_singleton, try_ensure_one, unreal::util, Result}, MemoryTrait};
use patternsleuth::scanner::Pattern;


define_pattern_resolver!(FString_AppendChars, [
    "45 85 C0 0F 84 89 00 00 00 48 89 5C 24 18 48 89 6C 24 20 56 48 83 EC 20 48 89 7C 24 30 48 8B EA 48 63 79 08 48 8B D9 4C 89 74 24 38 45 33 F6 85 FF 49 63 F0 41 8B C6 0F 94 C0 03 C7 03 C6 89 41 08 3B 41 0C 7E 07 8B D7 E8 ?? ?? ?? ?? 85 FF 49 8B C6 48 8B CF 48 8B D5 0F 95 C0 48 2B C8 48 8B 03 48 8D 1C 36 4C 8B C3 48 8D 3C 48 48 8B CF E8 ?? ?? ?? ?? 48 8B 6C 24 48 66 44 89 34 3B 4C 8B 74 24 38 48 8B 7C 24 30 48 8B 5C 24 40 48 83 C4 20 5E C3" // Universal
]);

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PreLogin(pub usize);
impl_resolver_singleton!(all, PreLogin, |ctx| async {
    // let patterns = [
    //     // "4C 89 4C 24 20 48 89 54 24 10 48 89 4C 24 08 55 53 57 41 55 41 57 48 8D 6C 24 C0 48 81 EC 40 01 00 00 4C 8B E9 4D 8B F9 49 8B 49 08 49 8B D8 48 8B FA 48 85 C9 0F 84 DB 06 00 00 48 8B 01 FF 50 ?? 84 C0 0F 84 CD 06 00 00 49 83 BD B0 02 00 00 00 0F 84 BF 06 00 00 49 8B 9D C0 04 00 00 33 FF 49 63 85 C8 04 00 00 48 89 B4 24 80 01 00 00 4C 89 A4 24 38 01 00 00 4C 89 B4 24 30 01 00 00 4C 8D 34 40 0F 29 B4 24 20 01 00 00 49 C1 E6 04 4C 03 F3 0F 29 BC 24 10 01 00 00 49 3B DE 0F 84 FC 05 00 00 48 8B 4B 08 48 85 C9 74 0F 48 8B 01 FF 50 ?? 84 C0 74 05 40 B6 01 EB 03 40 32 F6 49 8B 4F 08 48 85 C9 74 0E 48 8B 01 FF 50 ?? 84 C0 74 04 B0 01 EB 02 32 C0 40 3A F0 75 16 40 84 F6 74 1F 48 8B 4B 08 49 8B 57 08 48 8B 01 FF 10 84 C0 75 0E 48 83 C3 30 49 3B DE 75 A8 E9 9F 05 00 00 48 8D 4D B8 E8 ?? ?? ?? ?? 0F 57 F6 0F 57 FF 48 8B 08 48 8B 43 28 48 2B C1 F2 48 0F 2A F0 0F 28 C6 F2 0F 59 05 ?? ?? ?? ?? 66 0F 2F C7 0F 86 DE 04 00 00 BA 09 00 00 00 48 89 7C 24 58 48 8D 4C 24 58 48 89 7C 24 60 E8 ?? ?? ?? ?? 8B", // EGS
    //     // FIXME: this sig is somewhere in the middle of a function?
    //     "40 55 53 41 54 41 56 41 57 48 8D 6C 24 D1 48 81 EC D0 00 00 00 4C 8B F9 4D 8B F1 49 8B 49 08 49 8B D8 4C 8B E2 48 85 C9 0F 84 31 06 00 00 48 8B 01 FF 50 ?? 84 C0 0F 84 23 06 00 00 49 83 BF B0 02 00 00 00 0F 84 15 06 00 00 49 8B 9F C0 04 00 00 49 63 87 C8 04 00 00 48 89 B4 24 00 01 00 00 4C 89 AC 24 10 01 00 00 45 33 ED 48 89 BC 24 08 01 00 00 48 8D 34 40 0F 29 B4 24 C0 00 00 00 48 C1 E6 04 48 03 F3 0F 29 BC 24 B0 00 00 00 48 3B DE 0F 84 57 05 00 00 48 8B 4B 08 48 85 C9 74 0F 48 8B 01 FF 50 ?? 84 C0 74 05 40 B7 01 EB 03 40 32 FF 49 8B 4E 08 48 85 C9 74 0E 48 8B 01 FF 50 ?? 84 C0 74 04 B0 01 EB 02 32 C0 40 3A F8 75 16 40 84 FF 74 1F 48 8B 4B 08 49 8B 56 08 48 8B 01 FF 10 84 C0 75 0E 48 83 C3 30 48 3B DE 75 A8 E9 FA 04 00 00 48 8D 4D 77 E8 ?? ?? ?? ?? 0F 57 F6 0F 57 FF 48 8B 08 48 8B 43 28 48 2B C1 F2 48 0F 2A F0 0F 28 C6 F2 0F 59 05 ?? ?? ?? ?? 66 0F 2F C7 0F 86 88 04 00 00 BA 09 00 00 00 4C 89 6D 9F 48 8D 4D 9F 4C 89 6D A7 E8 ?? ?? ?? ?? 8B 55 A7 8B 5D AB 89 5D 77 8D 72 09 89 75 A7 3B F3 7E 12 48 8D 4D 9F E8 ?? ?? ?? ?? 8B 45 AB 8B 75 A7 89 45 77 48 8B 4D 9F 4C 8D 05 ?? ?? ?? ?? 41 B9 09 00 00 00 66 C7 44 24 20 3F 00 41 8B D1 E8 ?? ?? ?? ?? F2 0F 59 35 ?? ?? ?? ?? 0F 57 C0 48 B8 00 00 00 00 00 00 00 80 66 0F 2F F7 F2 48 0F 2C CE 73 23 48 3B C8 74 4A 0F 57 C0 F2 48 0F 2A C1 66 0F 2E C6 74 3C 66 0F 14 F6 66 0F 50 C6 83 E0 01 48 2B C8 EB 24 48 3B C8 74 27 0F 57 C0 F2 48 0F 2A C1 66 0F 2E C6 74 19 66 0F 14 F6 66 0F 50 C6 83 E0 01 83 F0 01 48 03 C8 0F 57 F6 F2 48 0F 2A F1 41 B8 01 00 00 00 48 8D 4D BF 0F 28 CE E8 ?? ?? ?? ?? 48 89 45 EF 4C 89 6D 8F 8B 58 08 85 DB 74 1E FF CB 44 89 6D 9B 41 8B D5 89 5D B7 49 8B FD 89 55 97 45 8B E5 8D 43 1C 85 C0 7E 28 EB 10 44 89 6D B7 41 8B DD 4C 89 6D 97 B8 1C 00 00 00 8B D0 48 8D 4D 8F E8 ?? ?? ?? ?? 8B 55 97 44 8B 65 9B 48 8B 7D 8F 44 8D 6A 1C 44 03 EB 44 89 6D 97 45 3B EC 7E 15 48 8D 4D 8F E8 ?? ?? ?? ?? 44 8B 65 9B 44 8B 6D 97 48 8B 7D 8F 0F 10 05 ?? ?? ?? ?? 48 63 5D B7 48 8D 4F 36 48 8B 55 EF 0F 11 07 0F 10 0D ?? ?? ?? ?? 4C 8D 04 1B 0F 11 4F 10 0F 10 05 ?? ?? ?? ?? 0F 11 47 20 8B 05 ?? ?? ?? ?? 89 47 30 0F B7 05 ?? ?? ?? ?? 66 89 47 34 48 8B 12 E8 ?? ?? ?? ?? 33 C0 66 89 44 5F 36 41 83 FD 01 7F 09 4C 8B 65 9F 44 8B E8 EB 4B 85 F6 74 04 FF CE EB 02 8B F0 42 8D 14 2E 48 89 7D 8F 44 89 6D 97 48 8B F8 44 89 65 9B 41 3B D4 7E 09 48 8D 4D 8F E8 ?? ?? ?? ?? 4C 8B 6D 9F 48 8D 4D 8F 49 8B D5 44 8B C6 E8 ?? ?? ?? ?? 8B 45 9B 4C 8B 65 8F 8B 75 97 89 45 77 48 8B 5D 7F 48 8D 45 F7 48 3B D8 74 1C 48 8B 0B 48 85 C9 74 05 E8 ?? ?? ?? ?? 8B 45 77 4C 89 23 45 33 E4 89 73 08 89 43 0C 4D 85 E4 74 08 49 8B CC E8 ?? ?? ?? ?? 48 85 FF 74 08 48 8B CF E8 ?? ?? ?? ?? 48 8B 4D BF 48 85 C9 74 05 E8 ?? ?? ?? ?? 4D 85 ED 74 08 49 8B CD E8 ?? ?? ?? ?? 4C 8B CB 48 8D 0D ?? ?? ?? ?? 4D 8B C6 49 8B D7 E8 ?? ?? ?? ?? 49 8B CF E8 ?? ?? ?? ?? 48 8B F8 48 85 C0 0F 84 C8 01 00 00 E8 ?? ?? ?? ?? 48 8B 4F 10 48 83 C0 30 48 63 50 08 3B 51 38 0F 8F AE 01 00 00 48 8B 49 30 48 39 04 D1 0F 85 A0 01 00 00 41 80 BF F8 00 00 00 03 75 2E 48 8B D3 48 8D 4D BF E8 ?? ?? ?? ?? 48 8B D0 49 8B CF E8 ?? ?? ?? ?? 48 8B 7D C7 48 85 FF 0F 84 72 01 00 00 BB FF FF FF FF E9 3F 01 00 00 E8 ?? ?? ?? ?? 48 8B F0 48 8D 4D 9F 33 C0 BA 05 00 00 00 48 89 45 9F 48 89 45 A7 E8 ?? ?? ?? ?? 8B 55 A7 8D 4A 05 89 4D A7 3B 4D AB 7E 09 48 8D 4D 9F E8 ?? ?? ?? ?? 48 8B 4D 9F 48 8D 15 ?? ?? ?? ?? 41 B8 0A 00 00 00 E8 ?? ?? ?? ?? 41 B8 01 00 00 00 48 8D 15 ?? ?? ?? ?? 48 8D 4D 77 E8 ?? ?? ?? ?? 4C 8D 4D 9F C6 44 24 20 01 48 8D 55 BF 48 8B CE 4C 8B 00 E8 ?? ?? ?? ?? 48 8B 4D 9F 48 85 C9 74 05 E8 ?? ?? ?? ?? E8 ?? ?? ?? ?? 48 8B D3 48 8D 4D 9F 48 8B F0 E8 ?? ?? ?? ?? 48 8D 8F C0 08 00 00 4C 8B F0 E8 ?? ?? ?? ?? 84 C0 74 1C 48 8D 8F D8 08 00 00 E8 ?? ?? ?? ?? 84 C0 74 0C 48 8D 8F F0 08 00 00 E8 ?? ?? ?? ?? 49 8B D6 48 8D 8F C0 08 00 00 E8 ?? ?? ?? ?? 48 8D 8F D8 08 00 00 48 8D 55 BF E8 ?? ?? ?? ?? 48 8D 8F F0 08 00 00 48 8B D6 E8 ?? ?? ?? ?? 48 8B 7D A7 BB FF FF FF FF 48 85 FF 74 2E 8B C3 F0 0F C1 47 08 83 F8 01 75 22 48 8B 07 48 8B CF FF 10 8B C3 F0 0F C1 47 0C 83 F8 01 75 0E 48 8B 07 BA 01 00 00 00 48 8B CF FF 50 ?? 48 8B 7D C7 48 85 FF 74 29 8B C3 F0 0F C1 47 08 83 F8 01 75 1D 48 8B 07 48 8B CF FF 10 F0 0F C1 5F 0C 83 FB 01 75 0B 48 8B 07 8B D3 48 8B CF FF 50 ?? 0F 28 B4 24 C0 00 00 00 48 8B BC 24 08 01 00 00 0F 28 BC 24 B0 00 00 00 48 8B B4 24 00 01 00 00 4C 8B AC 24 10 01 00 00 48 81 C4 D0 00 00 00 41 5F 41 5E 41 5C 5B 5D C3 48 8B 43 08 48 89 45 C7 48 8B 43 10 48 89 45 CF 48 85 C0 74 03 FF 40 08 48 8D 05 ?? ?? ?? ?? 4C 89 6D D7 48 89 45 BF 48 8D 55 BF 48 8B 43 28 49 8B CF 48 89 45 E7 4C 89 6D DF E8 ?? ?? ?? ?? 49 8B 8F B0 02 00 00 48 8D 55 BF 4D 8B C4 48 8B 01 FF 90 ?? ?? ?? ?? 48 8B 7D 7F 48 8B D8 48 3B F8 74 26 48 8B 0F 48 85 C9 74 05 E8 ?? ?? ?? ?? 48 8B 0B 48 89 0F 4C 89 2B 8B 43 08 89 47 08 8B 43 0C 89 47 0C 4C 89 6B 08 48 8B 4D BF 48 85 C9 74 05 E8 ?? ?? ?? ?? 4C 8B CF 48 8D 0D ?? ?? ?? ?? 4D 8B C6 49 8B D7 E8 ?? ?? ?? ?? E9 18 FF FF FF 4D 8B CE 4C 8B C3 49 8B D4 49 8B CF 48 81 C4 D0 00 00 00 41 5F 41 5E 41 5C 5B 5D E9 ?? ?? ?? ??", // STEAM
    //     // From Sigga
    //     "4C 89 4C 24 20 48 89 54 24 10 48 89 4C 24 08 55 53 57 41 55 41 57 48 8D 6C", // PDB
    // ];
    // let res = join_all(patterns.iter().map(|p| ctx.scan(Pattern::new(p).unwrap()))).await;

    // Ok(PreLogin(ensure_one(res.into_iter().flatten())?))
    
    // TODO: more strings in the function
    let strings = ctx.scan(util::utf8_pattern(" Minutes")).await;
    let refs = util::scan_xrefs(ctx, &strings).await;
    let mut fns = util::root_functions(ctx, &refs)?;
    if fns.len() == 2 {
        fns[0] = fns[1]; // FIXME: deque? last?
        fns.pop();
    }
    Ok(Self(ensure_one(fns)?))
});

define_pattern_resolver!(ApproveLogin, [
    "48 89 5C 24 18 48 89 74 24 20 55 57 41 54 41 55 41 56 48 8D 6C 24 C9 48 81 EC A0 00 00 00 8B", // EGS
    "48 89 5C 24 10 48 89 74 24 18 55 57 41 54 41 56 41 57 48 8B EC 48 81 EC 80 00 00 00 8B", // STEAM
]);


// TODO: Shorter sigs
define_pattern_resolver!(GetMotd, {
    EGS: ["4C 89 4C 24 20 4C 89 44 24 18 48 89 4C 24 08 55 56 57 41 54 48 8D 6C 24 C1 48 81 EC D8 00 00 00 83 79 08 01 4C 8B E2 48 8B F9 7F 19 33 F6 48 8B C2 48 89 32 48 89 72 08 48 81 C4 D8 00 00 00 41 5C 5F 5E 5D C3 48 89 9C 24 08 01 00 00 48 8D 55 B7 4C 89 AC 24 D0 00 00 00 4C 89 B4 24 C8 00 00 00 4C 89 BC 24 C0 00 00 00 E8 ?? ?? ?? ?? 4C 8B 6D B7 48 8D 4D 97 33 F6 48 89 75 97 48 89 75 9F 49 8B 45 00 8D 56 09"],
    STEAM: ["4C 89 4C 24 20 4C 89 44 24 18 48 89 4C 24 08 55 56 57 41 54 48 8D 6C 24 C1 48 81 EC E8 00 00 00 83 79 08 01 4C 8B E2 48 8B F9 7F 19 33 F6 48 8B C2 48 89 32 48 89 72 08 48 81 C4 E8 00 00 00 41 5C 5F 5E 5D C3 48 89 9C 24 18 01 00 00 48 8D 55 B7 4C 89 AC 24 E0 00 00 00 4C 89 B4 24 D8 00 00 00 4C 89 BC 24 D0 00 00 00 E8 ?? ?? ?? ?? 4C 8B 6D B7 48 8D 4C 24 20 33 F6 BA 09"]
});



#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct GetCurrentGames(pub usize);
impl_resolver_singleton!(all, GetCurrentGames, |ctx| async {
let patterns = [
        "E8 | ?? ?? ?? ?? 4C 39 ?8 74 3?" // Universal
    ];

    let res = join_all(patterns.iter().map(|p| ctx.scan(Pattern::new(p).unwrap()))).await;
    // let val = try_ensure_one(
    //     res.iter()
    //         .flatten()
    //         .map(|a| -> Result<usize> { Ok(ctx.image().memory.rip4(*a)?) })
    //         // .chain(res1),
    // );
    
    Ok(GetCurrentGames(try_ensure_one(
        res.iter()
            .flatten()
            .map(|a| -> Result<usize> { Ok(ctx.image().memory.rip4(*a)?) }))?))
});

define_pattern_resolver!(SendRequest, [
    "48 89 5C 24 ?? 48 89 74 24 ?? 48 89 7C 24 ?? 55 41 54 41 55 41 56 41 57 48 8B EC 48 83 EC 40 48 8B D9 49 8B F9"
]);