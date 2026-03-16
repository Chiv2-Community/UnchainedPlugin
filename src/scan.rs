use std::collections::HashMap;

use patternsleuth::resolvers::NamedResolver;

use std::process;
use anyhow::{Context, Result};
use crate::{resolvers::{self, PLATFORM, PlatformType, current_platform}, sdebug, sinfo};

pub fn scan(platform: PlatformType, resolvers_to_scan: Vec<&'static NamedResolver>) -> Result<HashMap<String, u64>> {
    let pid = process::id() as i32;

    if PLATFORM.get().is_none() {
        let _ = PLATFORM.set(platform);
    } else if PLATFORM.get().unwrap() != &platform {
        anyhow::bail!("Cannot scan for signatures on platform {:?} while running on {:?}", platform, current_platform());
    }

    if resolvers_to_scan.is_empty() {
        println!("All signatures already found");
        return Ok(HashMap::new());
    }

    sinfo!(f; "Scanning for {} missing signatures", resolvers_to_scan.len());
    resolvers_to_scan.iter().for_each(|res| sinfo!(f; "  {}", res.name));

    let dyn_resolvers = resolvers_to_scan.iter()
        .map(|res| res.getter)
        .collect::<Vec<_>>();

    let exe = patternsleuth::process::internal::read_image()
        .context("Failed to read image")?;

    let game_name = std::env::current_exe()
        .ok()
        .and_then(|path| path.file_name().map(|name| name.to_string_lossy().into_owned()))
        .map(|name| format!("{} (PID={})", name, pid))
        .unwrap_or_else(|| format!("PID={}", pid));

    sdebug!(f;"GAME '{:?}' '{:x?}'", game_name, exe.base_address);

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
                let val = u64::from_str_radix(hex.trim_start_matches("0x"), 16)
                    .context("Failed to parse hex offset")?;
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