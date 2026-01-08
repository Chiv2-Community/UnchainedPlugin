#[macro_use]
mod macros;
mod resolvers;
mod scan;
mod tools;
mod ue;
mod ue_old;
mod game;
mod features;

use once_cell::sync::Lazy;
use serenity::all::{ChannelId, CreateMessage};
use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::os::raw::c_char;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::to_writer_pretty;
#[cfg(feature="cli_commands")]
use crate::features::commands::spawn_cli_handler;
use crate::features::discord_bot::{DiscordBridge, DiscordConfig, OutgoingEvent};
#[cfg(feature="rcon_commands")]
use crate::features::rcon::handle_rcon;
use crate::features::server_registration::Registration;
use crate::resolvers::unchained_integration::CHAT_QUEUE;
use crate::tools::hook_globals::{globals, init_globals};
use crate::tools::misc::CLI_LOGO;
use self::resolvers::PlatformType;

// IEEE
use std::arch::x86_64::{_mm_crc32_u64, _mm_crc32_u8};

#[target_feature(enable = "sse4.2")]
unsafe fn crc32_from_file(path: &str) -> std::io::Result<u32> {
    let file = File::open(path)?;
    let mmap = memmap2::Mmap::map(&file)?;
    let mut crc: u64 = 0;

    let mut chunks = mmap.chunks_exact(8);
    for chunk in chunks.by_ref() {
        crc = _mm_crc32_u64(crc, u64::from_le_bytes(chunk.try_into().unwrap()));
    }

    for &byte in chunks.remainder() {
        crc = _mm_crc32_u8(crc as u32, byte) as u64;
    }

    Ok((crc as u32) ^ 0xFFFFFFFF)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct BuildInfo {
    build: u32,
    file_hash: u32,
    name: String,
    platform: PlatformType,
    path: String,
    offsets: HashMap<String, u64>,
}

static CURRENT_BUILD_INFO: Lazy<Mutex<Option<BuildInfo>>> = Lazy::new(|| Mutex::new(None));

fn expand_env_path(path: &str) -> Option<PathBuf> {
    if let Some(stripped) = path.strip_prefix("%LOCALAPPDATA%") {
        if let Ok(base) = env::var("LOCALAPPDATA") {
            return Some(PathBuf::from(base).join(stripped.trim_start_matches(['\\', '/'])));
        }
    }
    None
}

fn get_build_path(crc: u32, platform_type: PlatformType) -> Option<PathBuf> {
    let platform_str = platform_type.to_string();
    expand_env_path(&format!(
        r"%LOCALAPPDATA%\Chivalry 2\Saved\Config\{}-{:08x}.build.json",
        platform_str, crc
    ))
}

impl BuildInfo {
    pub fn scan(crc32: u32, platform: PlatformType) -> Self {
        println!("Scanning build...");

        let offsets = scan::scan(platform, None).expect("Failed to scan");

        let mut file_path = String::new();
        match env::current_exe() {
            Ok(path) => file_path = path.to_string_lossy().into(),
            Err(e) => eprintln!("Failed to get path: {}", e),
        }

        BuildInfo {
            build: 0,
            file_hash: crc32,
            name: "".to_string(),
            platform,
            path: file_path.to_string(),
            offsets,
        }
    }

    pub fn load(crc: u32, platform_type: PlatformType) -> Result<Self> {
        let path = get_build_path(crc, platform_type).context("Failed to expand path")?;
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let build_info: BuildInfo = serde_json::from_reader(reader)?;
        Ok(build_info)
    }

    pub fn save(&self) -> Result<()> {
        let path = get_build_path(self.file_hash, self.platform)
            .ok_or_else(|| anyhow::anyhow!("Failed to expand path"))?;
        sinfo!(f; "Saving build info to {}", path.to_string_lossy());

        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        to_writer_pretty(&mut writer, self)?;
        writer.flush()?;

        Ok(())
    }

    pub fn get_file_hash(&self) -> u32 {
        self.file_hash
    }

    pub fn get_offset(&self, name: &str) -> Option<&u64> {
        self.offsets.get(name)
    }

    pub fn get_offsets(&self) -> &HashMap<String, u64> {
        &self.offsets
    }

    pub fn add_offset(&mut self, name: String, offset: u64) {
        self.offsets.insert(name, offset);
    }
}

#[no_mangle]
pub extern "C" fn load_current_build_info(scan_missing: bool) -> *const BuildInfo {
    
    let mut current = CURRENT_BUILD_INFO.lock().unwrap();

    sdebug!(f; "Loading current build info, scan_missing={}", scan_missing);
    
    if current.is_none() {
        let file_path = env::current_exe()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();

        let crc32 = unsafe { crc32_from_file(&file_path) }.expect("Failed to compute CRC");

        let platform = match env::args().any(|arg| arg == "-epicapp=Peppermint") {
            true => PlatformType::EGS,
            false => PlatformType::STEAM,
        };

        match BuildInfo::load(crc32, platform) {
            Ok(bi) => {
                // println!("Loaded build info from cache");
                *current = Some(bi);
            }
            Err(err) => {
                eprintln!("Failed to load build info: {}", err);
                if scan_missing {
                    *current = Some(BuildInfo::scan(crc32, platform));
                }
            }
        }
    }

    if let (true, Some(bi)) = (scan_missing, current.as_mut()) {
        match scan::scan(bi.platform, Some(bi.get_offsets())) {
            Ok(new_offsets) if !new_offsets.is_empty() => {
                println!(
                    "Found {} missing signatures, updating build info",
                    new_offsets.len()
                );
                for (name, offset) in new_offsets {
                    bi.add_offset(name, offset);
                }
            }
            Ok(_) => {}
            Err(e) => eprintln!("Failed to scan for missing signatures: {}", e),
        }
    }
    
    let exe = patternsleuth::process::internal::read_image().map_err(|e| e.to_string()).expect("failed to read image");
    match current.as_ref() {
        None => sdebug!(f; "No current BuildInfo"),
        Some(bi) => {
            // Attach hooks
            
            let offsets = bi.offsets.clone();
            unsafe {
                apply_patches(exe.base_address, offsets.clone());
            }
            unsafe {
                attach_hooks(exe.base_address, offsets.clone()).unwrap();
            }
        },
    }

    // let pdb_file = r"U:\Games\Chivalry2_c\TBL\Binaries\Win64\Chivalry2-Win64-Shipping.pdb";    
    // tools::pdb_scan::list_functions_with_addresses(pdb_file, exe.base_address).expect("Failed to list functions");
    // swarn!(f; "{:#?}", globals());

    current
        .as_ref()
        .map(|bi| bi as *const BuildInfo)
        .unwrap_or(std::ptr::null())
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn build_info_save(bi: *const BuildInfo) -> u8 {
    let bi = unsafe { &*bi };
    if let Err(e) = bi.save() {
        eprintln!("Failed to save build info: {}", e);
        return 0;
    }
    1
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn build_info_get_file_hash(bi: *const BuildInfo) -> u32 {
    let bi = unsafe { &*bi };
    bi.get_file_hash()
}

#[no_mangle]
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn build_info_get_offset(bi: *const BuildInfo, name: *const c_char) -> u64 {
    let bi = unsafe { &*bi };
    let name = unsafe { std::ffi::CStr::from_ptr(name) }.to_string_lossy();
    *bi.get_offset(name.as_ref()).unwrap_or(&0)
}

// Initialize Logger and Globals
#[no_mangle]
pub extern "C" fn init_rustlib() {
    print!("{CLI_LOGO}");
    tools::logger::init_syslog().expect("Failed to init syslog");
    unsafe {
        init_globals().expect("Failed to init globals!");
    };
}

// pub fn start_discord_listener() {
//     std::thread::spawn(move || {
//         loop {
//             // Borrow the bridge from your globals
//             if let Some(bridge) = globals().DISCORD_BRIDGE.get() {
//                 // Drain all pending messages from the channel
//                 while let Ok(msg) = bridge.incoming.try_recv() {
//                     let ingame_text = format!("<Discord>{}: {}", msg.user, msg.text);
//                     sinfo!(f; "Received from disc: {ingame_text}");
//                     // Push to the shared queue
//                     if let Ok(mut queue) = CHAT_QUEUE.lock() {
//                         queue.push(ingame_text);
//                     }
//                 }
//             }
//             // Sleep to avoid pegging the CPU
//             std::thread::sleep(std::time::Duration::from_millis(100));
//         }
//     });
// }

#[no_mangle]
pub extern "C" fn postinit_rustlib() {
    #[cfg(feature="cli_commands")]
    spawn_cli_handler();
    #[cfg(feature="rcon_commands")]
    std::thread::spawn(|| {
        handle_rcon();
    });

    #[cfg(feature="server_registration")]
    {
        let args = &globals().cli_args;
        if args.rcon_port.is_some() || args.register {
            let query_port = args.game_server_query_port.unwrap_or(7071);
            let reg = Arc::new(Registration::new("127.0.0.1", query_port));
            
            let mut global_reg = globals().registration.lock().unwrap();
            *global_reg = Some(Arc::clone(&reg));
            
            sinfo!(f; "Started server registration manager");
            reg.start();
        }
    }
    
    #[cfg(feature="mod_management")]
    {
        use crate::features::mod_management::ModManager;

        let mm = Arc::new(ModManager::new());
        let mut global_mm = globals().mod_manager.lock().unwrap();
        *global_mm = Some(Arc::clone(&mm));
    }


    // #[cfg(feature="server_registration")]
    // {
    //     let args = &globals().cli_args;
    //     if args.rcon_port.is_some() || args.register {
    //         let reg = Arc::new(Registration::new(
    //             "127.0.0.1",
    //             globals().cli_args.game_server_query_port.unwrap()
    //         ));
    //         sinfo!(f; "Started server registration");
    //         reg.start();
    //     }
    // }

    
    #[cfg(feature="discord_integration")]
    if globals().cli_args.rcon_port.is_some() {
        // 1. Startup
        let config = DiscordConfig {
            bot_token: "TOKEN".to_string(),
            channel_id: 1154039134843846737,
        };
        let temp_bridge = DiscordBridge::new(config);
        let global_bridge = &globals().DISCORD_BRIDGE;
        let _ = global_bridge.set(temp_bridge).ok();
        
        if let Some(bridge) = global_bridge.get() {
            // if let Ok(msg) = bridge.incoming.try_recv() {
            //     let ingame_text = format!("<Discord>{}: {}", msg.user, msg.text);
                
            //     if let Some(world) = globals().world() {
            //         let mut txt = FText::default();
            //         use crate::{game::{chivalry2::EChatType, engine::FText}, ue::FString};
            //         let mut settings_fstring = FString::from(ingame_text.as_str());
            //         let res = unsafe { TRY_CALL_ORIGINAL!(FText_AsCultureInvariant(&mut txt, &mut settings_fstring)) } as *mut FText;
            //         let game_mode = TRY_CALL_ORIGINAL!(GetTBLGameMode(world));
            //         TRY_CALL_ORIGINAL!(BroadcastLocalizedChat(game_mode, res, EChatType::Admin));
            //     }
            // }

            // 3. Triggering events
            // Use a closure instead of a fn
            let on_player_win = |winner: &str, map_name: &str| {
                bridge.send_event(OutgoingEvent::MatchWon {
                    winner_name: winner.to_string(),
                    map: map_name.to_string(),
                });
            };

            // Call it like a function
            // on_player_win("Player1", "FFA_Wardenglade");
        }

        // start_discord_listener();
    }

}

use std::sync::OnceLock;

// // Global static storage for the bridge
// static DISCORD_BRIDGE: OnceLock<DiscordBridge> = OnceLock::new();

// // In your initialization:
// let bridge = DiscordBridge::new(config);
// let _ = DISCORD_BRIDGE.set(bridge);

// // Now your function works anywhere in the file:
// fn on_player_win(winner: &str, map_name: &str) {
//     if let Some(bridge) = DISCORD_BRIDGE.get() {
//         bridge.send_event(OutgoingEvent::MatchWon {
//             winner_name: winner.to_string(),
//             map: map_name.to_string(),
//         });
//     }
// }
/// # Safety
pub unsafe fn attach_hooks(
    base_address: usize,
    offsets: HashMap<String, u64>,
) -> Result<(), Box<dyn std::error::Error>> {
    sdebug!(f; "Attaching hooks via auto-discovery:");

    // inventory::iter finds everything submitted via CREATE_HOOK!
    for hook in inventory::iter::<resolvers::HookRegistration> {
        if !hook.auto_activate {
            sdebug!(f; "inactive hook: {}", hook.name);
            // Inactive hooks initialize but don't enable the detour
            // continue;
        }

        match (hook.hook_fn)(base_address, offsets.clone(), hook.auto_activate) {
            Ok(_) => sinfo!(f; "☑ {} {}", hook.name, if hook.auto_activate { "attached" } else { "attached (passive)" }),
            Err(e) => serror!(f; "☐ {}: {}", hook.name.to_uppercase(), e),
        }
    }

    Ok(())
}

/// # Safety
pub unsafe fn apply_patches(base: usize, offsets: std::collections::HashMap<String, u64>) {
    for p in inventory::iter::<resolvers::PatchRegistration> {
        // Run the condition check
        if (p.enabled_fn)() {
            match (p.patch_fn)(base, offsets.clone()) {
                Ok(_) => sinfo!(f; "[+] Patch Applied: {} ({})", p.name, p.tag),
                Err(e) => serror!(f; "[-] Patch Failed: {} ({}) -> {}", p.name, p.tag, e),
            }
        } else {
            sdebug!(f; "[.] Patch Skipped (Condition not met): {} ({})", p.name, p.tag);
        }
    }
}