use std::collections::HashMap;

use patternsleuth::resolvers::resolvers;

use std::process;

pub fn scan() -> Result<HashMap<String, u64>, String> {
    let pid = Some(process::id() as i32);

    let resolvers = resolvers().collect::<Vec<_>>();
    let dyn_resolvers = resolvers.iter().map(|res| res.getter).collect::<Vec<_>>();

    let name = format!("PID={}", pid.unwrap());
    let game_name = format!("pid={}", pid.unwrap()); // fixme
    let exe = patternsleuth::process::internal::read_image().map_err(|e| e.to_string())?;
    println!("GAME '{:?}' '{:x?}'", name, exe.base_address);

    let resolution = tracing::info_span!("scan", game = game_name)
        .in_scope(|| exe.resolve_many(&dyn_resolvers));

    // get Names and offsets from resolution
    let mut offsets = HashMap::new();
    for (resolver, resolution) in resolvers.iter().zip(&resolution) {
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
                println!("{} {} {} {:x?}", resolver.name, hex, val, (val-base) & 0xFFFFFFF);
                offsets.insert(resolver.name.to_string(), (val-base) & 0xFFFFFFF);
            }
        }
    } 

    // let res = dump_builds(offsets);

    Ok(offsets)
}