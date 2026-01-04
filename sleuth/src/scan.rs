use std::collections::HashMap;

use patternsleuth::resolvers::resolvers;

use std::process;
use crate::{resolvers::{self, PLATFORM, PlatformType, current_platform}, sdebug, sinfo};

pub fn scan(platform: PlatformType, existing_offsets: Option<&HashMap<String, u64>>) -> Result<HashMap<String, u64>, String> {
    let pid = process::id() as i32;

    if PLATFORM.get().is_none() {
        let _ = PLATFORM.set(platform);
    } else if PLATFORM.get().unwrap() != &platform {
        return Err(format!("Cannot scan for signatures on platform {:?} while running on {:?}", platform, current_platform()));
    }

    let resolvers = resolvers().collect::<Vec<_>>();

    // Filter resolvers to only scan for missing signatures
    let resolvers_to_scan: Vec<_> = if let Some(existing) = existing_offsets {
        resolvers.iter()
            .filter(|res| !existing.contains_key(res.name))
            .collect()
    } else {
        resolvers.iter().collect()
    };

    if resolvers_to_scan.is_empty() {
        println!("All signatures already found in cache");
        return Ok(HashMap::new());
    }

    sinfo!(f; "Scanning for {} missing signatures", resolvers_to_scan.len());
    resolvers_to_scan.iter().for_each(|res| sdebug!(f; "  {}", res.name));

    let dyn_resolvers = resolvers_to_scan.iter()
        .map(|res| res.getter)
        .collect::<Vec<_>>();

    let name = format!("PID={}", pid);
    let game_name = format!("pid={}", pid); // fixme
    let exe = patternsleuth::process::internal::read_image().map_err(|e| e.to_string())?;
    sdebug!(f;"GAME '{:?}' '{:x?}'", name, exe.base_address);

    let resolution = tracing::info_span!("scan", game = game_name)
        .in_scope(|| exe.resolve_many(&dyn_resolvers));

    // get Names and offsets from resolution
    let mut offsets = HashMap::new();
    for (resolver, resolution) in resolvers_to_scan.iter().zip(&resolution) {
        if let Ok(r) = resolution {
            // FIXME: Less nasty way?
            if let Some(hex) = format!("{r:?}")
                .split(['(', ')'])
                .nth(1)
                .and_then(|s| s.parse::<u64>().ok())
                .map(|n| format!("{:#x}", n))
            {
                // sigs_json.insert(MyItem { id: resolver.name.to_string(), name: hex.to_string() });
                let val = u64::from_str_radix(hex.trim_start_matches("0x"), 16).map_err(|e| e.to_string())?;
                let base = exe.base_address as u64;
                sinfo!(f; "{} {} {} {:x?}", resolver.name, hex, val, (val-base) & 0xFFFFFFF);
                offsets.insert(resolver.name.to_string(), (val-base) & 0xFFFFFFF);
            }
        }
    }
    for p in inventory::iter::<resolvers::OffsetRegisty> {
        let map: HashMap<String, u64> = (p.map)();
        sinfo!(f; "Offset Registry '{}': {} entries", p.name, map.len());
        for (k, v) in map.iter() {
            sinfo!(f; "  '{}' => 0x{:x}", k, v);
        }
        offsets.extend(map.into_iter());
    }

    Ok(offsets)
}