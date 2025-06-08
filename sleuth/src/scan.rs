use std::collections::HashMap;

use patternsleuth::resolvers::{resolvers, unreal::game_loop::UGameEngineTick, NamedResolver, Resolution};
use serde::{Deserialize, Serialize};

use std::process;

// pub static RESOLUTION: 
// Vec<Result<
// std::sync::Arc<dyn Resolution>, 
// patternsleuth::resolvers::ResolveError
// >> = Vec::new();

use once_cell::sync::{Lazy, OnceCell};
use std::sync::Arc;
use patternsleuth::resolvers::{ResolveError};


pub static OFFSETS: OnceCell<HashMap<String, usize>> = OnceCell::new();


pub fn scan() -> Result<HashMap<String, u64>, String> {
    let pid = Some(process::id() as i32);

    let resolvers = resolvers().collect::<Vec<_>>();
    let dyn_resolvers = resolvers.iter().map(|res| res.getter).collect::<Vec<_>>();

    // let name = format!("PID={}", pid.unwrap());
    let game_name = format!("pid={}", pid.unwrap()); // fixme
    let exe = patternsleuth::process::internal::read_image().map_err(|e| e.to_string())?;
    
    let resolution = tracing::info_span!("scan", game = game_name)
        .in_scope(|| exe.resolve_many(&dyn_resolvers));

    let mut offsets: HashMap<String, u64> = HashMap::new();
    let mut offsets_resolver: HashMap<String, usize> = HashMap::new();
    
    // FIXME: Nihi: ugh
    for (resolver, resolution) in resolvers.iter().zip(&resolution) {
        if let Ok(r) = resolution {
            // FIXME: Nihi: Less nasty way?
            if let Some(hex) = format!("{r:?}")
                .split(['(', ')'])
                .nth(1)
                .and_then(|s| s.parse::<u64>().ok())
                .map(|n| format!("{:#x}", n))
            {
                // sigs_json.insert(MyItem { id: resolver.name.to_string(), name: hex.to_string() });
                let val = u64::from_str_radix(hex.trim_start_matches("0x"), 16).map_err(|e| e.to_string())?;
                let base = exe.base_address as u64;
                // println!("{} {} {} {:x?}", resolver.name, hex, val, (val-base) & 0xFFFFFFF);
                offsets.insert(resolver.name.to_string(), (val-base) & 0xFFFFFFF);
                offsets_resolver.insert(resolver.name.to_string(), val as usize);
                // let ptr = Arc::as_ptr(resolver) as *const ();
                // offsets_resolver.insert(resolver as *const (), val as usize);
            }
        }
    } 

    let _ = OFFSETS.set(offsets_resolver); // Safe, only allowed once

    Ok(offsets) // Return the original u64-based map
}
