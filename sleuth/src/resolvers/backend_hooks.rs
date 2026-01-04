use patternsleuth::{MemoryTrait, resolvers::unreal::util};

use crate::{CREATE_REQUEST_HOOK, CREATE_REQUEST_HOOK_DUMMY, tools};

define_pattern_resolver!(FString_AppendChars, [
    "45 85 C0 0F 84 89 00 00 00 48 89 5C 24 18 48 89 6C 24 20 56 48 83 EC 20 48 89 7C 24 30 48 8B EA 48 63 79 08 48 8B D9 4C 89 74 24 38 45 33 F6 85 FF 49 63 F0 41 8B C6 0F 94 C0 03 C7 03 C6 89 41 08 3B 41 0C 7E 07 8B D7 E8 ?? ?? ?? ?? 85 FF 49 8B C6 48 8B CF 48 8B D5 0F 95 C0 48 2B C8 48 8B 03 48 8D 1C 36 4C 8B C3 48 8D 3C 48 48 8B CF E8 ?? ?? ?? ?? 48 8B 6C 24 48 66 44 89 34 3B 4C 8B 74 24 38 48 8B 7C 24 30 48 8B 5C 24 40 48 83 C4 20 5E C3" // Universal
]);

define_pattern_resolver!(
    PreLogin,
    XrefLast,
    [patternsleuth::resolvers::unreal::util::utf8_pattern(
        " Minutes"
    )]
);

define_pattern_resolver!(ApproveLogin, [
    "48 89 5C 24 18 48 89 74 24 20 55 57 41 54 41 55 41 56 48 8D 6C 24 C9 48 81 EC A0 00 00 00 8B", // EGS
    "48 89 5C 24 10 48 89 74 24 18 55 57 41 54 41 56 41 57 48 8B EC 48 81 EC 80 00 00 00 8B", // STEAM
]);


// TODO: Shorter sigs
// define_pattern_resolver!(GetMotd, {
//     EGS: ["4C 89 4C 24 20 4C 89 44 24 18 48 89 4C 24 08 55 56 57 41 54 48 8D 6C 24 C1 48 81 EC D8 00 00 00 83 79 08 01 4C 8B E2 48 8B F9 7F 19 33 F6 48 8B C2 48 89 32 48 89 72 08 48 81 C4 D8 00 00 00 41 5C 5F 5E 5D C3 48 89 9C 24 08 01 00 00 48 8D 55 B7 4C 89 AC 24 D0 00 00 00 4C 89 B4 24 C8 00 00 00 4C 89 BC 24 C0 00 00 00 E8 ?? ?? ?? ?? 4C 8B 6D B7 48 8D 4D 97 33 F6 48 89 75 97 48 89 75 9F 49 8B 45 00 8D 56 09"],
//     STEAM: [
//         "4C 89 4C 24 20 4C 89 44 24 18 48 89 4C 24 08 55 56 57 41 54 48 8D 6C 24 C1 48 81 EC E8 00 00 00 83 79 08 01 4C 8B E2 48 8B F9 7F 19 33 F6 48 8B C2 48 89 32 48 89 72 08 48 81 C4 E8 00 00 00 41 5C 5F 5E 5D C3 48 89 9C 24 18 01 00 00 48 8D 55 B7 4C 89 AC 24 E0 00 00 00 4C 89 B4 24 D8 00 00 00 4C 89 BC 24 D0 00 00 00 E8 ?? ?? ?? ?? 4C 8B 6D B7 48 8D 4C 24 20 33 F6 BA 09",
//         "4C 89 4C 24 20 4C 89 44 24 18 48 89 4C 24 08 55 56 57 41 54 48 8D 6C 24 C1 48 81 EC D8 00 00 00 83 79 08 01 4C 8B E2 48 8B F9 7F 19 33 F6 48 8B C2 48 89 32 48 89 72 08 48 81 C4 D8 00 00 00 41 5C 5F 5E 5D C3 48 89 9C 24 08 01 00 00 48 8D 55 B7 4C 89 AC 24 D0 00 00 00 4C 89 B4 24 C8 00 00 00 4C 89 BC 24 C0 00 00 00 E8 ?? ?? ?? ?? 4C 8B 6D B7 48 8D 4D 97 33 F6 48 89 75 97 48 89 75 9F 49 8B 45 00 8D 56 09" // EGS 2.11.4
//         ]
// });
// CREATE_REQUEST_HOOK!(GetMotd, "/api/tbio");

// define_pattern_resolver!(GetCurrentGames, Call, ["E8 | ?? ?? ?? ?? 4C 39 ?8 74 3?"]);
// CREATE_REQUEST_HOOK!(GetCurrentGames, "/api/tbio");

define_pattern_resolver!(SendRequest, [
    "48 89 5C 24 ?? 48 89 74 24 ?? 48 89 7C 24 ?? 55 41 54 41 55 41 56 41 57 48 8B EC 48 83 EC 40 48 8B D9 49 8B F9"
]);


// OpenAPIClientApi::CreateHttpRequest(OpenAPIClientApi *this,TSharedRef<> *__return_storage_ptr__,Request *param_1)
use std::collections::HashMap;
use std::sync::RwLock;
use once_cell::sync::Lazy;

pub static STRING_TO_ROOT_FN: Lazy<RwLock<HashMap<String, u64>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

define_pattern_resolver!(CreateHttpRequest, [
    "40 53 55 56 41 56 48 83 EC 78 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 44 24",
], |ctx, patterns| {
    let futures = ::patternsleuth::resolvers::futures::future::join_all(
        patterns.iter().map(|p| ctx.scan(::patternsleuth::scanner::Pattern::new(p).unwrap()))
    ).await;

    let addr = futures.into_iter().flatten().next().ok_or_else(|| {
        ::patternsleuth::resolvers::ResolveError::Msg(
            format!("Failed to find match for CreateHttpRequest with patterns: {patterns:?}", patterns = patterns).into()
        )
    })?;

    let base_addr = crate::globals().get_base_address();
    let mem = &ctx.image().memory;
    let str_offset = 0x49 + 0x3; // offset to LEA call + string addr

    let mut calls = Vec::new();
    let mut strings = Vec::new();

    for call in util::scan_xcalls(ctx, [&addr]).await {
        let str_addr = mem.rip4(call + str_offset).unwrap_or(0);
        if let Ok(s) = mem.read_wstring(str_addr) {
            calls.push(call);
            let suffix = s.rsplit('/').next().map(|s| s.to_string()).unwrap_or(s);
            strings.push(format!("{}_{}", "GetPost", suffix));
        }
    }

    let roots = util::root_functions(ctx, &calls)?;

    {
        let mut map = STRING_TO_ROOT_FN.write().expect("STRING_TO_ROOT_FN poisoned");
        for (s, root) in strings.into_iter().zip(roots.into_iter()) {
            // crate::sinfo!(f; "CreateHttpRequest mapping: '{}' => 0x{:x}", s, (root - base_addr) as u64);
            if root >= base_addr { map.insert(s, (root - base_addr) as u64); }
        }
    }

    {
        let map = STRING_TO_ROOT_FN.read().unwrap();
        crate::sinfo!(f; "CreateHttpRequest string → root map:");
        for (s, fn_addr) in map.iter() {
            crate::sinfo!(f; "  '{}' => 0x{:x}", s, fn_addr);
        }
    }
    
    inventory::submit! {
        crate::resolvers::OffsetRegisty {
            name: "CreateHttpRequest Get*Post hooks",
            map: || STRING_TO_ROOT_FN.read().unwrap().clone(),
        }
    }

    Ok(addr)
});


CREATE_REQUEST_HOOK!(GetPost_GetMotd, "/api/tbio");
CREATE_REQUEST_HOOK!(GetPost_GetCurrentGames, "/api/tbio");

CREATE_REQUEST_HOOK_DUMMY!(GetPost_GetPlayerInventory);
CREATE_REQUEST_HOOK_DUMMY!(GetPost_LoginPlayFab);
CREATE_REQUEST_HOOK_DUMMY!(GetPost_GetCampaigns);
CREATE_REQUEST_HOOK_DUMMY!(GetPost_ReAuthServerCustomId);
CREATE_REQUEST_HOOK_DUMMY!(GetPost_PostLogin);
CREATE_REQUEST_HOOK_DUMMY!(GetPost_Heartbeat);
CREATE_REQUEST_HOOK_DUMMY!(GetPost_PreRegisterGame);
CREATE_REQUEST_HOOK_DUMMY!(GetPost_GetMyProgress);
CREATE_REQUEST_HOOK_DUMMY!(GetPost_GetJoinTicket);
CREATE_REQUEST_HOOK_DUMMY!(GetPost_DeregisterGame);
CREATE_REQUEST_HOOK_DUMMY!(GetPost_NotifyPlayerJoined);
CREATE_REQUEST_HOOK_DUMMY!(GetPost_GrantFlavorEntitlement);
CREATE_REQUEST_HOOK_DUMMY!(GetPost_ReportTelemetry);
CREATE_REQUEST_HOOK_DUMMY!(GetPost_PhonebookSearchId);
CREATE_REQUEST_HOOK_DUMMY!(GetPost_PhonebookGetOwnId);
CREATE_REQUEST_HOOK_DUMMY!(GetPost_Award);
CREATE_REQUEST_HOOK_DUMMY!(GetPost_RedeemEntitlementEOS);
// CREATE_REQUEST_HOOK_DUMMY!(GetPost_GetCurrentGames);
// CREATE_REQUEST_HOOK_DUMMY!(GetPost_GetMotd);
CREATE_REQUEST_HOOK_DUMMY!(GetPost_GrantPendingRewards);
CREATE_REQUEST_HOOK_DUMMY!(GetPost_PurchaseWithVirtualCurrency);
CREATE_REQUEST_HOOK_DUMMY!(GetPost_SetCampaignPurchased);
CREATE_REQUEST_HOOK_DUMMY!(GetPost_GetTitleData);
CREATE_REQUEST_HOOK_DUMMY!(GetPost_LoginEOS);
CREATE_REQUEST_HOOK_DUMMY!(GetPost_ConfirmJoinTicket);
CREATE_REQUEST_HOOK_DUMMY!(GetPost_PhonebookDeleteOwnId);

// define_pattern_resolver!(CreateHttpRequest, [
//     "40 53 55 56 41 56 48 83 EC 78 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 44 24",
//     // "40 53 55 56 41 56 48 83 EC 78 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 44 24 60 33 DB"
// ],
// |ctx, patterns| {
//             let futures = ::patternsleuth::resolvers::futures::future::join_all(
//                 patterns.iter()
//                     .map(|p| ctx.scan(::patternsleuth::scanner::Pattern::new(p).unwrap()))
//             ).await;
    
//             let res = futures
//                 .into_iter()
//                 .flatten()
//                 .next()
//                 .ok_or_else(||
//                     ::patternsleuth::resolvers::ResolveError::Msg(format!(
//                         "Failed to find match for asdf with one of the patterns: {patterns:?}",
//                          patterns = patterns
//                     ).into())
//             );  
//             let base_addr = crate::globals().get_base_address();
//             let addr: usize = res.clone()?;
            
//             let xcalls = util::scan_xcalls(ctx, [&addr]).await;
//             crate::sinfo!(f; "CreateHttpRequest xcalls found: {}", xcalls.len());
//             // let xrefs = util::scan_xrefs(ctx, [&addr]).await;
//             // crate::sinfo!(f; "CreateHttpRequest xrefs found: {}", xrefs.len());
//             let str_offset = 0x49 + 0x3;       
//             let str_next = 0x49 + 0x3 + 0x4;       
//             let mem = &ctx.image().memory;
//             let mut fns = util::root_functions(ctx, &xcalls)?;
//             crate::sinfo!(f; "CreateHttpRequest root functions found: {}", fns.len());
//             for call in xcalls {
//                 let str_ref_addr = call + str_offset;
//                 let str_addr = mem.rip4(str_ref_addr).unwrap_or(0);
//                 crate::sinfo!(f; 
//                     "CreateHttpRequest xcall string ref at 0x{:x}, string addr at 0x{:x}, next instr at 0x{:x}",
//                     str_ref_addr - base_addr,
//                     str_addr - base_addr,
//                     call + str_next - base_addr);
//                 // match mem.read_string(mem.ptr(str_addr)?) {
//                 match mem.read_wstring(str_addr) {
//                     Ok(s) => {
//                         crate::sinfo!(f; "CreateHttpRequest xcall string at 0x{:x}: {}", str_addr, s);
//                     },
//                     Err(e) => {
//                         crate::sinfo!(f; "CreateHttpRequest xcall string read error at 0x{:x}: {}", str_addr, e);
//                     }
//                 }
//             }
//             for fn_addr in &fns {
//                 if *fn_addr >= base_addr {
//                     // crate::sinfo!(f; "CreateHttpRequest use found at: 0x{:x}", *fn_addr - base_addr);
//                     let pdb_file = r"U:\Games\Chivalry2_c\TBL\Binaries\Win64\Chivalry2-Win64-Shipping.pdb";    
//                     let rva = (*fn_addr - base_addr) as u32;
//                     match tools::pdb_scan::get_function_name_from_rva(pdb_file, rva as u32) {
//                         Ok(Some(name)) => {
//                             crate::sinfo!(f; "Function name: {}", name);
//                         },
//                         Ok(None) => {
//                             crate::sinfo!(f; "No function name found for RVA {:#X}", rva);
//                         },
//                         Err(e) => {
//                             crate::sinfo!(f; "Error reading PDB: {}", e);
//                         }
//                     }
//                 }
//             }
//             crate::sinfo!(f; "CreateHttpRequest found at: 0x{:x}", res.as_ref().unwrap() - base_addr);
//             res
//         });