//! IPv6 browser fixes at four call sites, never at the shared FString entry points.

mod callsite;
mod endpoint;

use std::{collections::HashMap, sync::OnceLock};

use crate::{
    tools::hook_globals::cli_args,
    ue::{FString, TArray},
};
use callsite::CallPatch;
use endpoint::{ipv6_port_separator, is_ipv6_host, split_ipv6_endpoint};

// These entry points are called directly for fallback; neither is detoured.
define_pattern_resolver!(FStringFind, [
    "48 89 5C 24 10 48 89 74 24 18 48 89 7C 24 20 41 54 41 56 41 57 48 83 EC 50 45 8B E1 48 8B FA 4C 8B F1"
]);
define_pattern_resolver!(
    FStringParseIntoArray,
    ["44 88 4C 24 20 53 55 56 57 41 55 41 57 48 83 EC 28 48 8B FA 48 8B D9"]
);

// Matches the installed Steam executable and the older PDB build. The marker
// resolves the E8 instruction, not the containing function or its callee.
define_pattern_resolver!(Ipv6BrowserFind, [
    "45 33 C9 C7 44 24 20 FF FF FF FF 48 8D 15 ?? ?? ?? ?? 49 8B CE 45 8D 41 01 | E8 ?? ?? ?? ?? 83 F8 FF 74 ?? 33 C0 4C 8D 05 ?? ?? ?? ?? 41 B1 01 48 89 45 10",
    "45 33 C9 C7 44 24 20 FF FF FF FF 48 8D 15 ?? ?? ?? ?? 48 8B CE 45 8D 41 01 | E8 ?? ?? ?? ?? 83 F8 FF 74 ?? 41 B1 01 48 89 7D 10"
]);
define_pattern_resolver!(
    Ipv6PingFind,
    ["48 8B CF 48 0F 45 D6 45 33 C9 45 8D 41 01 | E8 ?? ?? ?? ?? 8B 4F 08 85 C9 74 05 48 8B 3F"]
);
define_pattern_resolver!(Ipv6PendingPingFind, [
    "49 0F 45 D5 49 8D 4F 50 45 33 C9 45 8D 41 01 | E8 ?? ?? ?? ?? 41 8B 4F 58 85 C9 0F 84 ?? ?? ?? ?? 4D 8B 7F 50"
]);
define_pattern_resolver!(Ipv6UdpSplit, [
    "89 4C 24 70 44 0F 28 FB 48 8B CB F3 44 0F 11 45 B0 89 44 24 34 89 45 88 | E8 ?? ?? ?? ?? 33 D2 89 44 24 40 8B 43 08 48 8B 1B"
]);

type Find = unsafe extern "C" fn(*const FString, *const u16, i32, i32, i32) -> i32;
type Split = unsafe extern "C" fn(*const FString, *mut TArray<FString>, *const u16, bool) -> i32;

struct Originals {
    find: Find,
    split: Split,
}

static ORIGINALS: OnceLock<Originals> = OnceLock::new();
static INSTALLED: OnceLock<Result<(), String>> = OnceLock::new();

unsafe fn is_colon(text: *const u16) -> bool {
    !text.is_null() && *text == b':' as u16 && *text.add(1) == 0
}

unsafe extern "C" fn browser_find(
    haystack: *const FString,
    needle: *const u16,
    search_case: i32,
    search_direction: i32,
    start_position: i32,
) -> i32 {
    if !haystack.is_null() && is_colon(needle) && is_ipv6_host(&(*haystack).to_string()) {
        // The address field is a host, not host:port. Skip the destructive split;
        // OnGetCurrentGames will append its separate PINGPORT_i afterward.
        return -1;
    }
    (ORIGINALS.get().unwrap().find)(
        haystack,
        needle,
        search_case,
        search_direction,
        start_position,
    )
}

unsafe extern "C" fn ping_find(
    haystack: *const FString,
    needle: *const u16,
    search_case: i32,
    search_direction: i32,
    start_position: i32,
) -> i32 {
    if !haystack.is_null() && is_colon(needle) {
        if let Some(separator) = ipv6_port_separator(&(*haystack).to_string()) {
            return separator;
        }
    }
    (ORIGINALS.get().unwrap().find)(
        haystack,
        needle,
        search_case,
        search_direction,
        start_position,
    )
}

unsafe extern "C" fn udp_split(
    haystack: *const FString,
    output: *mut TArray<FString>,
    delimiter: *const u16,
    cull_empty: bool,
) -> i32 {
    if !haystack.is_null() && !output.is_null() && is_colon(delimiter) {
        let endpoint = (*haystack).to_string();
        if let Some((host, port)) = split_ipv6_endpoint(&endpoint) {
            let mut parts = TArray::with_capacity(2);
            parts.push(FString::from(host));
            parts.push(FString::from(port));
            // Uses the engine allocator; assignment also releases any previous output.
            *output = parts;
            return 2;
        }
    }
    (ORIGINALS.get().unwrap().split)(haystack, output, delimiter, cull_empty)
}

unsafe fn install(offsets: HashMap<String, u64>) -> Result<(), Box<dyn std::error::Error>> {
    INSTALLED
        .get_or_init(|| {
            let address = |name: &str| -> Result<usize, String> {
                let offset = offsets
                    .get(name)
                    .ok_or_else(|| format!("Missing {name} offset"))?;
                let offset = usize::try_from(*offset).map_err(|_| "Offset overflow".to_string())?;
                crate::resolvers::BASE_ADDR
                    .checked_add(offset)
                    .ok_or_else(|| "Address overflow".into())
            };
            let find = address("FStringFind")?;
            let split = address("FStringParseIntoArray")?;
            let calls = [
                CallPatch {
                    address: address("Ipv6BrowserFind")?,
                    expected_target: find,
                    replacement: browser_find as *const () as usize,
                },
                CallPatch {
                    address: address("Ipv6PingFind")?,
                    expected_target: find,
                    replacement: ping_find as *const () as usize,
                },
                CallPatch {
                    address: address("Ipv6PendingPingFind")?,
                    expected_target: find,
                    replacement: ping_find as *const () as usize,
                },
                CallPatch {
                    address: address("Ipv6UdpSplit")?,
                    expected_target: split,
                    replacement: udp_split as *const () as usize,
                },
            ];
            ORIGINALS
                .set(Originals {
                    find: std::mem::transmute::<usize, Find>(find),
                    split: std::mem::transmute::<usize, Split>(split),
                })
                .map_err(|_| "IPv6 originals already initialized".to_string())?;
            callsite::install(&calls)
        })
        .clone()
        .map_err(Into::into)
}

inventory::submit! {
    crate::resolvers::PatchRegistration {
        name: "BrowserIpv6Callsites",
        tag: "endpoint_parsing",
        patch_fn: install,
        enabled_fn: || !cli_args().is_server(),
    }
}
