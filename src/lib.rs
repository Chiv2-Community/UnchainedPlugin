
#[macro_use]
pub mod macros;
mod resolvers;
pub mod scan;
pub mod tools;
mod ue;
pub mod game;
pub mod features;
pub mod commands;
pub mod discord;
#[cfg(windows)]
mod seh;

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::time::Duration;
use std::{env, thread};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::to_writer_pretty;
#[cfg(feature="cli_commands")]
use crate::commands::spawn_cli_handler;
use crate::discord::config::DiscordConfig;
#[cfg(feature="rcon_commands")]
use crate::features::rcon::handle_rcon;
use crate::features::server_registration::Registration;
use crate::game::chivalry2::EChatType;
use crate::tools::hook_globals::{CLI_ARGS, cli_args, globals, init_globals};
use crate::tools::misc::CLI_LOGO;
use self::resolvers::PlatformType;



fn crc32_from_file(path: &str) -> std::io::Result<u32> {
    let file = File::open(path)?;
    let mmap = unsafe { memmap2::Mmap::map(&file)? };
    Ok(crc32c::crc32c(&mmap))
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



use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::SystemServices::{DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH};
use windows::Win32::System::Threading::{CreateThread, THREAD_CREATION_FLAGS};

#[no_mangle]
#[allow(non_snake_case)]
pub unsafe extern "system" fn DllMain(
    _h_module: HMODULE,
    ul_reason_for_call: u32,
    _lp_reserved: *mut std::ffi::c_void,
) -> i32 {
    match ul_reason_for_call {
        DLL_PROCESS_ATTACH => {
            // preinit needs to happen very early, before the new thread starts
            preinit_rustlib();

            // We can't do much in DllMain due to loader lock, so we spawn a thread
            let _ = CreateThread(
                None,
                0,
                Some(main_thread_adapter),
                None,
                THREAD_CREATION_FLAGS(0),
                None,
            );
        }
        DLL_PROCESS_DETACH => {
            // Cleanup if needed
        }
        _ => (),
    }
    1
}

unsafe extern "system" fn main_thread_adapter(_: *mut std::ffi::c_void) -> u32 {
    let result = std::panic::catch_unwind(|| {
        rust_main();
    });
    if let Err(e) = result {
        eprintln!("Rust main thread panicked: {:?}", e);
    }
    0
}

fn rust_main() {
    init_rustlib();

    // Check if we need to scan for missing signatures
    let need_rescan = check_for_missing_signatures();

    if need_rescan {
        sinfo!(f; "Missing signatures detected, scanning...");
        // Rescan and update build info
        let _ = load_current_build_info(true);

        // Save the updated build info
        if let Some(bi) = CURRENT_BUILD_INFO.lock().unwrap().as_ref() {
            if let Err(e) = bi.save() {
                serror!(f; "Failed to save build info: {}", e);
            } else {
                sinfo!(f; "Build info saved. Please restart the application.");
            }
        }

        // Exit with code -67
        std::process::exit(-67);
    }

    postinit_rustlib();
    sinfo!(f; "Unchained Sleuth initialized.");
}

fn get_current_platform() -> PlatformType {
    match env::args().any(|arg| arg == "-epicapp=Peppermint") {
        true => PlatformType::EGS,
        false => PlatformType::STEAM,
    }
}

fn scan_for_missing_offsets(bi: &BuildInfo) -> Result<HashMap<String, u64>> {
    scan::scan(bi.platform, Some(bi.get_offsets()))
}

fn check_for_missing_signatures() -> bool {
    let current = CURRENT_BUILD_INFO.lock().unwrap();

    if let Some(bi) = current.as_ref() {
        // Check if any signatures are missing by attempting to scan
        match scan_for_missing_offsets(bi) {
            Ok(new_offsets) if !new_offsets.is_empty() => {
                sinfo!(f; "Found {} missing signatures", new_offsets.len());
                return true;
            }
            Ok(_) => return false,
            Err(e) => {
                serror!(f; "Failed to check for missing signatures: {}", e);
                return false;
            }
        }
    }

    // No build info loaded, need to scan
    true
}

fn load_current_build_info(scan_missing: bool) -> *const BuildInfo {

    let mut current = CURRENT_BUILD_INFO.lock().unwrap();

    sdebug!(f; "Loading current build info, scan_missing={}", scan_missing);

    if current.is_none() {
        let file_path = env::current_exe()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();

        let crc32 = crc32_from_file(&file_path).expect("Failed to compute CRC");
        let platform = get_current_platform();

        match BuildInfo::load(crc32, platform) {
            Ok(bi) => {
                sinfo!(f; "Loaded build info from cache");
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
        match scan_for_missing_offsets(bi) {
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

    // Apply patches and hooks if we have build info and haven't applied yet
    // Only apply when not scanning (i.e., during preinit)
    if !scan_missing && !APPLIED.load(Ordering::Relaxed) {
        match current.as_ref() {
            None => sdebug!(f; "No current BuildInfo"),
            Some(bi) => {
                // Attach hooks and apply patches
                let offsets = bi.offsets.clone();
                apply_patches(offsets.clone());
                if let Err(e) = attach_hooks(offsets.clone()) {
                    serror!(f; "Failed to attach hooks: {}", e);
                }
                APPLIED.store(true, Ordering::Relaxed);
                sinfo!(f; "Applied patches and hooks in preinit");
            },
        }
    }
    else if scan_missing {
        swarn!(f; "Skipping patch application during scan");
    }
    else {
        swarn!(f; "Patches already applied");
    }

    #[cfg(feature="with_pdb")]
    {
        // let pdb_file = r"U:\Games\Chivalry2_c\TBL\Binaries\Win64\Chivalry2-Win64-Shipping.pdb";
        // tools::pdb_scan::list_functions_with_addresses(pdb_file, exe.base_address).expect("Failed to list functions");
        // swarn!(f; "{:#?}", globals());
    }

    current
        .as_ref()
        .map(|bi| bi as *const BuildInfo)
        .unwrap_or(std::ptr::null())
}

use windows::Win32::System::Console::{AllocConsole, GetConsoleWindow, GetStdHandle, GetConsoleMode, SetConsoleMode, STD_OUTPUT_HANDLE, ENABLE_VIRTUAL_TERMINAL_PROCESSING, CONSOLE_MODE};

fn preinit_rustlib() {
    unsafe {
        if GetConsoleWindow().0 == 0 {
            let _ = AllocConsole();

            // Enable ANSI escape codes for colored output
            if let Ok(handle) = GetStdHandle(STD_OUTPUT_HANDLE) {
                let mut mode = CONSOLE_MODE(0);
                if GetConsoleMode(handle, &mut mode).is_ok() {
                    let _ = SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
                }
            }
        }
    }

    std::panic::set_hook(Box::new(|panic_info| {
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };
        let location = panic_info.location().map(|l| format!(" at {}:{}", l.file(), l.line())).unwrap_or_default();
        eprintln!("CRITICAL ERROR: {}{} - Application will exit in 10 seconds", message, location);
        std::thread::sleep(std::time::Duration::from_secs(10));
    }));

    let args = tools::cli_args::load_cli().expect("Failed to load CLI ARGS");
    sdebug!(f; "CLI Args: {:#?}", args);
    if CLI_ARGS.set(args).is_err() {
        eprintln!("Error: Cli args already initialized!");
    }
    tools::logger::init_syslog().expect("Failed to init syslog");

    // Load build info without scanning
    let _ = load_current_build_info(false);
}

// Initialize Logger and Globals
fn init_rustlib() {
    print!("{CLI_LOGO}");
    // tools::logger::init_syslog().expect("Failed to init syslog");
    init_globals().expect("Failed to init globals!");
}

static ENGINE_READY: AtomicBool = AtomicBool::new(false);
static WORLD_READY: AtomicBool = AtomicBool::new(false);

fn postinit_rustlib() {
    
    seh::install();
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

    thread::spawn(|| {
        crate::sinfo!("waiting for engine to start..");
        
        while !ENGINE_READY.load(Ordering::Relaxed) {
            thread::sleep(Duration::from_millis(500));
        }

        #[cfg(feature="cli_commands")]
        spawn_cli_handler();
        if cli_args().is_server() {
            world_init();
        }
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

/// The base program address of the running application will be used.
pub fn attach_hooks(
    offsets: HashMap<String, u64>,
) -> Result<(), Box<dyn std::error::Error>> {
    sdebug!(f; "Attaching hooks via auto-discovery:");

    // inventory::iter finds everything submitted via CREATE_HOOK!
    for hook in inventory::iter::<resolvers::HookRegistration> {
        let cond = (hook.condition)();
        if !cond {
            // swarn!(f; "inactive hook: {}", hook.name);
            // Inactive hooks initialize but don't enable the detour
            // continue;
        }

        match unsafe { (hook.hook_fn)(offsets.clone(), cond) } {
            Ok(_) => sinfo!(f; "☑ {} {}", hook.name, if cond { "attached" } else { "attached (passive)" }),
            Err(e) => serror!(f; "☐ {}: {}", hook.name.to_uppercase(), e),
        }
    }

    Ok(())
}

/// The base program address of the running application will be used.
pub fn apply_patches(offsets: std::collections::HashMap<String, u64>) {
    for p in inventory::iter::<resolvers::PatchRegistration> {
        // Run the condition check
        if (p.enabled_fn)() {
            match unsafe { (p.patch_fn)(offsets.clone()) } {
                Ok(_) => sinfo!(f; "[+] Patch Applied: {} ({})", p.name, p.tag),
                Err(e) => serror!(f; "[-] Patch Failed: {} ({}) -> {}", p.name, p.tag, e),
            }
        } else {
            sdebug!(f; "[.] Patch Skipped (Condition not met): {} ({})", p.name, p.tag);
        }
    }
}
