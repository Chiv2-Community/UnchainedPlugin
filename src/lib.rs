
#[macro_use]
pub mod macros;
mod resolvers;
pub mod scan;
pub mod tools;
mod ue;
pub mod game;
pub mod features;
pub mod commands;
#[cfg(windows)]
mod seh;
pub mod events;
pub mod modules;
#[cfg(test)]
pub mod test_utils;

use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use std::{env, thread};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use anyhow::{Context, Result};
use patternsleuth::resolvers::{resolvers, NamedResolver};
use serde::{Deserialize, Serialize};
use serde_json::to_writer_pretty;
#[cfg(feature="cli_commands")]
use crate::commands::spawn_cli_handler;
#[cfg(feature="rcon_commands")]
use crate::features::rcon::handle_rcon;
use crate::features::server_registration::Registration;
use crate::game::chivalry2::EChatType;
use crate::tools::hook_globals::{cli_args, globals, init_globals, CLI_ARGS};
use serenity::all::ChannelId;
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
    plugin_version: String,
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

static RESOLVERS_TO_IGNORE: Lazy<HashSet<&'static str>> = Lazy::new(||
    HashSet::from([
        "FTextFString", "AESKeys", "FFrameStepViaExec", "BlueprintLibraryInit",
        "EngineVersionStrings", "EngineVersion", "A", "UtilStringExtractor",
        "ConsoleManagerSingleton", "KismetSystemLibrary", "UGameplayStaticsSaveGameToMemory"
    ])
);

impl BuildInfo {
    pub fn load_or_create(crc: u32, platform_type: PlatformType) -> Self {
        let load_result =
            get_build_path(crc, platform_type)
                .context("Failed to expand path")
                .and_then(|path| File::open(path).context("Failed to open build info file"))
                .map(BufReader::new)
                .and_then(|reader| serde_json::from_reader(reader).context("Failed to deserialize build info"))
                .and_then(|result: BuildInfo|
                    if result.plugin_version == env!("CARGO_PKG_VERSION") { Ok(result) }
                    else { Err(anyhow::anyhow!("Build info version mismatch")) }
                )
                .map_err(|e| anyhow::anyhow!("Failed to load build info: {}", e));


        match load_result {
            Ok(bi) => bi,
            Err(e) => {
                swarn!(f; "Failed to load build info: {}", e);
                swarn!(f; "Initializing new build info.");
                BuildInfo {
                    plugin_version: env!("CARGO_PKG_VERSION").to_string(),
                    build: 0,
                    file_hash: crc,
                    name: "".to_string(),
                    platform: platform_type,
                    path: "".to_string(),
                    offsets: HashMap::new(),
                }
            }
        }
    }


    pub fn save(&self) -> Result<Self> {
        let path = get_build_path(self.file_hash, self.platform)
            .ok_or_else(|| anyhow::anyhow!("Failed to expand path"))?;
        sinfo!(f; "Saving build info to {}", path.to_string_lossy());

        let self_with_path = self.with_path(path.to_string_lossy().into_owned());

        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        to_writer_pretty(&mut writer, &self_with_path)?;
        writer.flush()?;

        Ok(self_with_path)
    }

    pub fn get_file_hash(&self) -> u32 {
        self.file_hash
    }

    pub fn get_offset(&self, name: &str) -> Option<&u64> {
        self.offsets.get(name)
    }

    pub fn get_unresolved(&self, resolvers_in: Vec<&'static NamedResolver>) -> Vec<&'static NamedResolver> {
        resolvers_in
            .into_iter()
            .filter(|resolver| !RESOLVERS_TO_IGNORE.contains(resolver.name))
            .filter(|resolver| !self.offsets.contains_key(resolver.name))
            .map(|resolver| resolver)
            .collect()
    }

    pub fn with_scan(&self, all_resolvers: Vec<&'static NamedResolver>) -> Result<Self> {
        scan::scan(self.platform, self.get_unresolved(all_resolvers))
            .map(|new_offsets| {
                let mut all_offsets = self.offsets.clone();
                all_offsets.extend(new_offsets);
                self.with_offsets(all_offsets)
            })
    }

    pub fn with_offsets(&self, offsets: HashMap<String, u64>) -> Self {
        let mut cloned = self.clone();
        cloned.offsets = offsets;
        cloned
    }

    pub fn with_additional_offsets(&self, offsets: HashMap<String, u64>) -> Self {
        let mut cloned = self.clone();
        cloned.offsets.extend(offsets);
        cloned
    }

    pub fn with_path(&self, path: String) -> Self {
        let mut cloned = self.clone();
        cloned.path = path;
        cloned
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

    let need_rescan = {
        let current = CURRENT_BUILD_INFO.lock().unwrap();
        current.is_none() || current.as_ref().unwrap().get_unresolved(resolvers().collect()).len() > 0
    };

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

fn load_current_build_info(scan_missing: bool) -> *const BuildInfo {

    let mut current = CURRENT_BUILD_INFO.lock().unwrap();

    sdebug!(f; "Loading current build info, scan_missing={}", scan_missing);

    if current.is_none() {
        let file_path = env::current_exe()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();

        let crc32 = crc32_from_file(&file_path).expect("Failed to compute CRC");
        let platform = get_current_platform();

        *current = Some(BuildInfo::load_or_create(crc32, platform));
    }


    if let (true, Some(bi)) = (scan_missing, current.as_ref()) {
        sinfo!("Scanning for missing signatures");
        match bi.with_scan(resolvers().collect()) {
            Ok(bi_with_scan_results) => *current = Some(bi_with_scan_results),
            Err(e) => eprintln!("Failed to scan for missing signatures: {}", e)
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

use windows::Win32::System::Console::{AllocConsole, GetConsoleMode, GetConsoleWindow, GetStdHandle, SetConsoleMode, CONSOLE_MODE, ENABLE_EXTENDED_FLAGS, ENABLE_QUICK_EDIT_MODE, ENABLE_VIRTUAL_TERMINAL_PROCESSING, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE};
use crate::events::models::{ChatSource, ChatType, GameChatMessage};
use crate::events::models::GameEvent::GameChatMessageEvent;
use crate::features::tokio_runtime::TOKIO_RUNTIME;
use crate::features::discord::DiscordConfig;
use crate::features::events::EVENT_SYSTEM;

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

            // Disable QuickEdit mode to prevent application freezing when console is clicked
            if let Ok(handle) = GetStdHandle(STD_INPUT_HANDLE) {
                let mut mode = CONSOLE_MODE(0);
                if GetConsoleMode(handle, &mut mode).is_ok() {
                    // We must clear ENABLE_QUICK_EDIT_MODE and also include ENABLE_EXTENDED_FLAGS to make it effective.
                    let _ = SetConsoleMode(handle, (mode & !ENABLE_QUICK_EDIT_MODE) | ENABLE_EXTENDED_FLAGS);
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

fn postinit_rustlib() {
    let _ = *crate::features::tokio_runtime::TOKIO_RUNTIME;
    // crate::features::events::init_event_system();
    unsafe { seh::install() };
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

    #[cfg(feature="cli_commands")]
    {
        if cli_args().is_server() {
            thread::spawn(move || {
                crate::sinfo!("waiting for engine to start..");
                
                while !ENGINE_READY.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(500));
                }

                spawn_cli_handler();
                if cli_args().is_server() {
                    world_init();
                }
            });
        } else {
            sinfo!(f; "Not a server, skipping server cli initialization");
        }
    }
}

pub fn world_init() {
    #[cfg(feature="rcon_commands")]
    thread::spawn(|| {
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
    while globals().world().is_none() {
        thread::sleep(Duration::from_millis(500));
    }

    if let Some(motd) = cli_args().motd.clone() {
        sinfo!(f; "Starting MOTD broadcast loop");
        thread::spawn(move || {
            loop {
                sinfo!(f; "Broadcasting MOTD: {}", motd);
                game::chivalry2::send_ingame_message(motd.clone(), Some(EChatType::ServerSay));
                thread::sleep(Duration::from_secs(30 * 60));
            }
        });
    }

    {
        TOKIO_RUNTIME.spawn(async move {
            let _ = features::events::initialize_subscribers().await;

            EVENT_SYSTEM.game_event_publisher.publish(GameChatMessageEvent(GameChatMessage {
                chat_source: ChatSource::Console,
                chat_type: ChatType::Global,
                sender: "Server".to_string(),
                message: "Initialized Event System".to_string(),
            }));

            if let Some(bot_token) = cli_args().discord_bot_token.clone() {
                let _discord_config = features::discord::initialize_discord_system(DiscordConfig {
                    bot_token,
                    dashboard_channel_id: cli_args().discord_dashboard_channel_id.map(ChannelId::new),
                    general_chat_channel_id: cli_args().discord_general_channel_id.map(ChannelId::new),
                    admin_notification_channel_id: cli_args().discord_admin_channel_id.map(ChannelId::new),
                    event_log_channel_id: cli_args().discord_event_log_channel_id.map(ChannelId::new),
                    admin_role_id: cli_args().discord_admin_role_id,
                    mention_on_admin: cli_args().discord_mention_admins,
                });

                EVENT_SYSTEM.game_event_publisher.publish(GameChatMessageEvent(GameChatMessage {
                    chat_source: ChatSource::Console,
                    chat_type: ChatType::Global,
                    sender: "Server".to_string(),
                    message: "Initialized Discord Event Handler System".to_string(),
                }));
            }

        });
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
