
#[macro_use]
pub mod macros;
mod resolvers;
pub mod scan;
pub mod tools;
mod ue;
mod ue_old;
pub mod game;
pub mod features;
pub mod commands;
pub mod discord;
#[cfg(windows)]
mod seh;



use once_cell::sync::Lazy;
use serenity::all::{ChannelId, CreateMessage};
use std::collections::HashMap;
use std::time::Duration;
use std::{env, thread};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::os::raw::c_char;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::to_writer_pretty;
#[cfg(feature="cli_commands")]
use crate::commands::spawn_cli_handler;
use crate::discord::config::DiscordConfig;
#[cfg(feature="cli_commands")]
// use crate::features::commands::spawn_cli_handler;
// use crate::features::discord_bot::{DiscordBridge, DiscordConfig, OutgoingEvent};
#[cfg(feature="rcon_commands")]
use crate::features::rcon::handle_rcon;
use crate::features::server_registration::Registration;
use crate::game::chivalry2::EChatType;
// use crate::resolvers::unchained_integration::CHAT_QUEUE;
use crate::tools::hook_globals::{CLI_ARGS, cli_args, globals, init_globals};
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
    
    static APPLIED: AtomicBool = AtomicBool::new(false);

    if !APPLIED.load(Ordering::Relaxed) {
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
                APPLIED.store(true, Ordering::Relaxed);
            },
        }
    }
    else {
        swarn!(f; "Patches already applied");
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

#[no_mangle]
pub extern "C" fn preinit_rustlib() {
    let args = unsafe { tools::cli_args::load_cli().expect("Failed to load CLI ARGS") };
    sdebug!(f; "CLI Args: {:#?}", args);
    if CLI_ARGS.set(args).is_err() {
        eprintln!("Error: Cli args already initialized!");
    }
    tools::logger::init_syslog().expect("Failed to init syslog");
}

// Initialize Logger and Globals
#[no_mangle]
pub extern "C" fn init_rustlib() {
    print!("{CLI_LOGO}");
    // tools::logger::init_syslog().expect("Failed to init syslog");
    unsafe {
        init_globals().expect("Failed to init globals!");
    };
}

static ENGINE_READY: AtomicBool = AtomicBool::new(false);
static WORLD_READY: AtomicBool = AtomicBool::new(false);

#[no_mangle]
pub extern "C" fn postinit_rustlib() {
    
    unsafe {
        seh::install();
    }
    // #[cfg(feature="cli_commands")]
    // spawn_cli_handler();
    // #[cfg(feature="rcon_commands")]
    // std::thread::spawn(|| {
    //     handle_rcon();
    // });

    // #[cfg(feature="server_registration")]
    // {
    //     let args = &cli_args();
    //     if args.is_server() || args.register {
    //         let query_port = args.game_server_query_port.unwrap_or(7071);
    //         let reg = Arc::new(Registration::new("127.0.0.1", query_port));
            
    //         let mut global_reg = globals().registration.lock().unwrap();
    //         *global_reg = Some(Arc::clone(&reg));
            
    //         sinfo!(f; "Started server registration manager");
    //         reg.start();
    //     }
    // }
    
    // #[cfg(feature="mod_management")]
    // {
    //     use crate::features::mod_management::ModManager;

    //     let mm = Arc::new(ModManager::new());
    //     let mut global_mm = globals().mod_manager.lock().unwrap();
    //     *global_mm = Some(Arc::clone(&mm));
    // }
    
    // let args = &cli_args();
    #[cfg(feature="discord_integration_old")]
    if args.discord_enabled() {
        
        // let config = DiscordConfig { 
        //     bot_token: args.discord_bot_token.clone().expect("Token invalid"),
        //     channel_id: args.discord_channel_id.unwrap()
        // };
        let global_bridge = &globals().DISCORD_BRIDGE;
        let _ = global_bridge.set(DiscordBridge::new(config)).ok();
        
        // if let Some(bridge) = global_bridge.get() {
        //     let on_player_win = |winner: &str, map_name: &str| {
        //         bridge.send_event(OutgoingEvent::MatchWon {
        //             winner_name: winner.to_string(),
        //             map: map_name.to_string(),
        //         });
        //     };
        // }
    }

    thread::spawn(|| {
        crate::sinfo!("waiting for engine to start..");
        
        while !ENGINE_READY.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(500));
        }

        world_init();
    });
}

struct GameChatSink;
impl discord::ChatSink for GameChatSink {
    fn send(&self, text: String, chat_type: discord::ChatType) {
        let game_chat_type = match chat_type {
            discord::ChatType::Admin => Some(EChatType::Admin),
            discord::ChatType::Global => Some(EChatType::AllSay),
            discord::ChatType::Team => Some(EChatType::TeamSay),
        };
        game::chivalry2::send_ingame_message(text, game_chat_type);
    }
}

pub fn world_init() {
    #[cfg(feature="cli_commands")]
    spawn_cli_handler();

    #[cfg(feature="rcon_commands")]
    std::thread::spawn(|| {
        handle_rcon();
    });

    let args = &cli_args();
    sinfo!(f; "Server: {}, Discord: {}", args.is_server(), args.discord_enabled());

    #[cfg(feature="server_registration")]
    {
        if args.is_server() || args.register {
            let query_port = args.game_server_query_port.unwrap_or(7071);
            let reg = Arc::new(Registration::new("127.0.0.1", query_port));
            
            let mut global_reg = globals().registration.lock().unwrap();
            *global_reg = Some(Arc::clone(&reg));
            
            sinfo!(f; "Started server registration manager");
            reg.start();
        }
    }

    // Mod manager requires world
    while !WORLD_READY.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(500));
    }

    if cli_args().discord_enabled() {
        sinfo!(f; "Starting discord bridge");
        // let config = DiscordConfig {
        //     bot_token: cli_args().discord_bot_token.clone().expect("Token invalid"),
        //     channel_id: cli_args().discord_channel_id.unwrap(),
        //     admin_channel_id: cli_args().discord_admin_channel_id.unwrap(),
        //     general_channel_id: cli_args().discord_general_channel_id.unwrap(),
        //     admin_role_id: 1113981344872140822,
        //     disabled_modules: vec![],
        //     blocked_notifications: vec![],
        //     modules: HashMap::default()
        // };
        
        fn update<T>(target: &mut T, source: Option<T>) {
            if let Some(val) = source {
                *target = val;
            }
        }

        let config_path = "discord_bot_config.json";
        let mut config = DiscordConfig::load(config_path, true).unwrap_or_else(|e| {
            serror!("Configuration Error, loading default: {}", e);
            DiscordConfig::default()
        });

        let cli = &cli_args();
        update(&mut config.bot_token, cli.discord_bot_token.clone());
        update(&mut config.channel_id, cli.discord_channel_id);
        update(&mut config.admin_channel_id, cli.discord_admin_channel_id);
        update(&mut config.general_channel_id, cli.discord_general_channel_id);
        update(&mut config.admin_role_id, cli.discord_admin_role_id);

        let ctx = Arc::new(discord::SleuthContext {
            chat: Arc::new(GameChatSink),
            config
        });
        
        // This spawns the background thread and the Tokio runtime
        let handle = crate::discord::DiscordBridge::init(config_path, ctx);

        // Store the handle globally so the dispatch! macro can find it
        crate::discord::DISCORD_HANDLE.set(handle)
            .expect("Discord Handle was already initialized!");

        sinfo!(f; "Discord Bridge is running in the background...");
    }
    
    #[cfg(feature="mod_management")]
    {
        thread::sleep(Duration::from_millis(200));
        use crate::features::mod_management::ModManager;

        let mm = Arc::new(ModManager::new());
        let mut global_mm = globals().mod_manager.lock().unwrap();
        *global_mm = Some(Arc::clone(&mm));

        sinfo!(f; "Mod scans in progress");
        mm.scan_asset_registry();
        mm.update_save_game();
        sinfo!(f; "Started mod manager");
    }
}

/// # Safety
pub unsafe fn attach_hooks(
    base_address: usize,
    offsets: HashMap<String, u64>,
) -> Result<(), Box<dyn std::error::Error>> {
    sdebug!(f; "Attaching hooks via auto-discovery:");

    // inventory::iter finds everything submitted via CREATE_HOOK!
    for hook in inventory::iter::<resolvers::HookRegistration> {
        let cond = (hook.condition)();
        if !cond {
            swarn!(f; "inactive hook: {}", hook.name);
            // Inactive hooks initialize but don't enable the detour
            // continue;
        }

        match (hook.hook_fn)(base_address, offsets.clone(), cond) {
            Ok(_) => sinfo!(f; "☑ {} {}", hook.name, if cond { "attached" } else { "attached (passive)" }),
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
