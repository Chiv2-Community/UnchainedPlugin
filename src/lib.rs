#![cfg_attr(feature = "server_registration", feature(ip))]
#![feature(arbitrary_self_types_pointers)]
#[macro_use]
extern crate unchained_macros_patternsleuth;
use unchained_macros_core::{SelectorIdentity, SelectorSnapshot};

pub mod platform;
pub use platform::{PlatformType, PLATFORM, current_platform};

#[macro_use]
pub mod logging;
pub mod scan;
pub mod tools;
pub mod Chivalry2;
mod unchained;
pub mod features;
pub mod commands;
#[cfg(windows)]
mod seh;
pub mod events;
pub mod modules;
#[cfg(test)]
pub mod test_utils;

use std::collections::HashMap;
use std::time::Duration;
use std::{env, thread};
use std::fs::{create_dir_all, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use anyhow::{Context, Result};
use patternsleuth::resolvers::NamedResolver;
use serde::{Deserialize, Serialize};
use serde_json::to_writer_pretty;
#[cfg(feature="cli_commands")]
use crate::commands::spawn_cli_handler;
#[cfg(feature="rcon_commands")]
use crate::features::rcon::handle_rcon;
#[cfg(feature="server_registration")]
use crate::features::server_browser::registration::Registration;
use crate::tools::hook_globals::{cli_args, globals, init_globals, CLI_ARGS};
use serenity::all::ChannelId;



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
    #[serde(default)]
    selectors: Vec<SelectorIdentity>,
    path: String,
    offsets: HashMap<String, u64>,
}

static CURRENT_BUILD_INFO: Mutex<Option<BuildInfo>> = Mutex::new(None);
static CURRENT_SELECTORS: OnceLock<SelectorSnapshot> = OnceLock::new();

fn selectors() -> &'static SelectorSnapshot {
    CURRENT_SELECTORS.get().expect("Selectors must be captured before startup")
}

/// Offset (relative to the game image base) resolved for `name` in the current
/// build info, if any.
pub fn current_build_offset(name: &str) -> Option<u64> {
    CURRENT_BUILD_INFO.lock().unwrap().as_ref()?.get_offset(name).copied()
}

fn expand_env_path(path: &str) -> Option<PathBuf> {
    if let Some(stripped) = path.strip_prefix("%LOCALAPPDATA%") {
        if let Ok(base) = env::var("LOCALAPPDATA") {
            return Some(PathBuf::from(base).join(stripped.trim_start_matches(['\\', '/'])));
        }
    }
    None
}

fn get_build_path(crc: u32, platform_type: PlatformType, selectors: &[SelectorIdentity]) -> Option<PathBuf> {
    let platform_str = platform_type.to_string();
    let selector_crc = crc32c::crc32c(&serde_json::to_vec(selectors).expect("selector identities serialize"));
    expand_env_path(&format!(
        r"%LOCALAPPDATA%\Chivalry 2\Saved\Config\{}-{:08x}-{:08x}.build.json",
        platform_str, crc, selector_crc,
    ))
}

fn resolvers(snapshot: &SelectorSnapshot) -> std::vec::IntoIter<&'static NamedResolver> {
    unchained_macros_patternsleuth::startup_resolvers(snapshot)
        .unwrap_or_else(|error| panic!("Invalid startup resolver registration: {error}"))
        .into_iter()
}

impl BuildInfo {
    pub fn load_or_create(crc: u32, snapshot: &SelectorSnapshot) -> Self {
        let platform_type = snapshot.get::<PlatformType>().expect("Platform selector is captured");
        let load_result =
            get_build_path(crc, platform_type, snapshot.cache_key())
                .context("Failed to expand path")
                .and_then(|path| File::open(path).context("Failed to open build info file"))
                .map(BufReader::new)
                .and_then(|reader| serde_json::from_reader(reader).context("Failed to deserialize build info"))
                .and_then(|result: BuildInfo|
                    if result.plugin_version == env!("CARGO_PKG_VERSION")
                        && result.file_hash == crc && result.platform == platform_type
                        && result.selectors == snapshot.cache_key()
                    { Ok(result) } else { Err(anyhow::anyhow!("Build info identity mismatch")) }
                )
                .map_err(|e| anyhow::anyhow!("Failed to load build info: {}", e));


        match load_result {
            Ok(bi) => bi,
            Err(e) => {
                swarn!("Failed to load build info: {}", e);
                swarn!("Initializing new build info.");
                BuildInfo {
                    plugin_version: env!("CARGO_PKG_VERSION").to_string(),
                    build: 0,
                    file_hash: crc,
                    name: "".to_string(),
                    platform: platform_type,
                    selectors: snapshot.cache_key().to_vec(),
                    path: "".to_string(),
                    offsets: HashMap::new(),
                }
            }
        }
    }


    pub fn save(&self) -> Result<Self> {
        let path = get_build_path(self.file_hash, self.platform, &self.selectors)
            .ok_or_else(|| anyhow::anyhow!("Failed to expand path"))?;
        sinfo!("Saving build info to {}", path.to_string_lossy());

        if let Some(parent) = path.parent() {
            create_dir_all(parent).context("Failed to create build info directory")?;
        }

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

    pub fn get_unresolved(&self, snapshot: &SelectorSnapshot, resolvers_in: Vec<&'static NamedResolver>) -> Vec<&'static NamedResolver> {
        resolvers_in
            .into_iter()
            .filter(|resolver| snapshot.supports_resolver_for::<unchained_macros_patternsleuth::PatternsleuthResolver>(resolver.name))
            .filter(|resolver| self.selectors != snapshot.cache_key() || !self.offsets.contains_key(resolver.name))
            .map(|resolver| resolver)
            .collect()
    }

    pub fn with_scan(&self, snapshot: &SelectorSnapshot, all_resolvers: Vec<&'static NamedResolver>) -> Result<Self> {
        anyhow::ensure!(self.selectors == snapshot.cache_key(), "Cannot mix selector snapshots in one address cache");
        scan::scan(snapshot, self.get_unresolved(snapshot, all_resolvers))
            .map(|new_offsets| self.with_additional_offsets(new_offsets))
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

#[cfg(test)]
mod build_info_tests {
    use super::*;

    fn unused_getter() -> &'static patternsleuth::resolvers::DynResolverFactory {
        panic!("cache identity checks must not scan");
    }
    static SYNTHETIC: NamedResolver = NamedResolver { name: "Synthetic", getter: unused_getter };

    #[test_case::test_case(false; "different selector values")]
    #[test_case::test_case(true; "legacy cache without selector identity")]
    fn incompatible_selector_caches_require_a_fresh_scan(legacy: bool) {
        let original = SelectorSnapshot::from_selection(PlatformType::STEAM).unwrap();
        let snapshot = SelectorSnapshot::from_selection(PlatformType::EGS).unwrap();
        let mut info = BuildInfo {
            plugin_version: env!("CARGO_PKG_VERSION").into(), build: 0, file_hash: 1,
            name: String::new(), platform: PlatformType::STEAM, path: String::new(),
            offsets: HashMap::from([("Synthetic".into(), 123)]),
            selectors: original.cache_key().to_vec(),
        };
        if legacy {
            let mut serialized = serde_json::to_value(&info).unwrap();
            serialized.as_object_mut().unwrap().remove("Selectors");
            info = serde_json::from_value(serialized).unwrap();
            assert!(info.selectors.is_empty());
        }
        assert_eq!(info.get_unresolved(&snapshot, vec![&SYNTHETIC]).len(), 1);
        assert!(info.with_scan(&snapshot, vec![&SYNTHETIC]).is_err());
    }

    #[test]
    fn unrelated_cache_entries_do_not_require_scanning() {
        let snapshot = SelectorSnapshot::from_selection(PlatformType::STEAM).unwrap();
        let info = BuildInfo {
            plugin_version: String::new(), build: 0, file_hash: 0,
            name: String::new(), platform: PlatformType::STEAM, path: String::new(),
            offsets: HashMap::from([("Unrelated".into(), 123)]),
            selectors: snapshot.cache_key().to_vec(),
        };
        assert!(info.get_unresolved(&snapshot, Vec::new()).is_empty());
    }

    #[test]
    fn incomplete_cache_requires_normal_rescan() {
        let snapshot = SelectorSnapshot::from_selection(PlatformType::STEAM).unwrap();
        let required: Vec<_> = resolvers(&snapshot).collect();
        let mut info = BuildInfo {
            plugin_version: String::new(), build: 0, file_hash: 0,
            name: String::new(), platform: PlatformType::STEAM, path: String::new(),
            offsets: required.iter().map(|resolver| (resolver.name.to_string(), 1)).collect(),
            selectors: snapshot.cache_key().to_vec(),
        };
        assert!(info.get_unresolved(&snapshot, required.clone()).is_empty());
        for resolver in &required {
            info.offsets.remove(resolver.name);
            let missing = info.get_unresolved(&snapshot, required.clone());
            assert!(!missing.is_empty());
            assert!(missing.iter().all(|missing| missing.name == resolver.name));
            info.offsets.insert(resolver.name.to_string(), 1);
        }
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
        serror!("Rust main thread panicked: {:?}", e);
    }
    0
}

fn rust_main() {
    let need_rescan = {
        let current = CURRENT_BUILD_INFO.lock().unwrap();
        current.as_ref().is_none_or(|info| !info.get_unresolved(selectors(), resolvers(selectors()).collect()).is_empty())
    };

    if need_rescan {
        sinfo!("Missing signatures detected, scanning...");
        load_current_build_info();
        let mut current = CURRENT_BUILD_INFO.lock().unwrap();
        if let Some(bi) = current.as_ref() {
            match bi.with_scan(selectors(), resolvers(selectors()).collect()) {
                Ok(scanned) => *current = Some(scanned),
                Err(error) => serror!("Failed to scan for missing signatures: {}", error),
            }
        }
        if let Some(bi) = current.as_ref() {
            if let Err(e) = bi.save() {
                serror!("Failed to save build info: {}", e);
            } else {
                sinfo!("Build info saved. Please restart the application.");
            }
        }

        // Exit with code -67
        std::process::exit(-67);
    }

    init_rustlib();
    postinit_rustlib();
    sinfo!("Unchained Sleuth initialized.");
}

fn load_current_build_info() {
    let mut current = CURRENT_BUILD_INFO.lock().unwrap();
    if current.is_none() {
        let file_path = env::current_exe()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();

        let crc32 = crc32_from_file(&file_path).expect("Failed to compute CRC");
        *current = Some(BuildInfo::load_or_create(crc32, selectors()));
    }
}

fn initialize_native_runtime() {
    static APPLIED: AtomicBool = AtomicBool::new(false);
    if APPLIED.load(Ordering::Relaxed) {
        swarn!("Patches already applied");
        return;
    }

    let current = CURRENT_BUILD_INFO.lock().unwrap();
    let bi = current.as_ref().expect("Build info must be loaded before native initialization");
    // Scan incomplete caches on the worker thread, outside DllMain's loader lock.
    let snapshot = selectors();
    if !bi.get_unresolved(snapshot, resolvers(snapshot).collect()).is_empty() {
        sinfo!("Missing signatures; deferring initialization until scan and restart");
        return;
    }

    let enabled = |name: &str| snapshot.supports_resolver_for::<unchained_macros_patternsleuth::PatternsleuthResolver>(name);
    let context = unchained_macros_patternsleuth::ScanContext::new(*unchained::startup::BASE_ADDR, &bi.offsets);
    unsafe { unchained_macros_core::initialize_value_bindings_where(*unchained::startup::BASE_ADDR, &bi.offsets, enabled); }
    if let Err(error) = unsafe { unchained_macros_core::initialize_bindings_for::<unchained_macros_patternsleuth::PatternsleuthResolver>(&context, snapshot) } {
        serror!("Failed to bind native functions: {}", error);
        std::process::exit(-67);
    }
    if let Err(error) = unsafe { apply_patches::<unchained_macros_patternsleuth::PatternsleuthResolver>(&context, snapshot, &mut unchained_macros_core::PatchState::default()) } {
        serror!("Failed to apply patches: {}", error);
        std::process::exit(-67);
    }
    if let Err(error) = attach_hooks(snapshot) {
        serror!("Failed to attach hooks: {}", error);
        std::process::exit(-67);
    }
    APPLIED.store(true, Ordering::Relaxed);
    sinfo!("Applied patches and hooks in preinit");
}

use crate::events::models::{ChatSource, ChatType, GameChatMessage};
use crate::events::models::GameEvent::GameChatMessageEvent;
use crate::features::tokio_runtime::TOKIO_RUNTIME;
use crate::features::discord::DiscordConfig;
use crate::features::events::EVENT_SYSTEM;

fn preinit_rustlib() {
    // Select output before any console allocation, color configuration, or banner.
    let args = tools::cli_args::load_cli().expect("Failed to load CLI ARGS");
    let already_initialized = CLI_ARGS.set(args).is_err();
    if cli_args().no_spawn_console {
        logging::console::disable_console_spawning();
    }
    if cli_args().attach_stdio {
        logging::console::attach_stdio();
    }
    logging::console::spawn_console();

    logging::log4rs::init().expect("Failed to initialize logging");
    sdebug!("CLI args: {:?}", cli_args());
    if already_initialized {
        serror!("CLI args already initialized!");
    }

    let platform = if env::args().any(|arg| arg == "-epicapp=Peppermint") {
        PlatformType::EGS
    } else {
        PlatformType::STEAM
    };
    PLATFORM.set(platform).expect("Platform already set");
    CURRENT_SELECTORS.set(SelectorSnapshot::capture_registered().expect("Failed to capture selector providers"))
        .expect("Selectors already captured");
    load_current_build_info();
    initialize_native_runtime();
}

// Initialize plugin globals on the worker thread.
fn init_rustlib() {
    logging::console::print_banner();
    init_globals();
}

static ENGINE_READY: AtomicBool = AtomicBool::new(false);

fn postinit_rustlib() {
    let _ = *crate::features::tokio_runtime::TOKIO_RUNTIME;
    unsafe { seh::install() };

    #[cfg(feature="cli_commands")]
    {
        // The console accepts commands for every instance; only servers run world_init.
        thread::spawn(move || {
            sinfo!("waiting for engine to start..");

            while !ENGINE_READY.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(500));
            }

            if !cli_args().is_server() {
                sinfo!("Not a server, skipping server initialization");
                // Clients get a mod manager so `mod-list` can scan the mod
                // markers on demand; nothing is scanned at startup.
                #[cfg(feature="mod_management")]
                {
                    use crate::features::mod_management::ModManager;
                    let mut mod_manager = globals().mod_manager.lock().unwrap();
                    if mod_manager.is_none() {
                        *mod_manager = Some(Arc::new(ModManager::new()));
                    }
                }
                // Clients still expose the client-safe diagnostic commands
                // (listmodmaps / mod-list) through the plugin console.
                // Spawn this before spawn_cli_handler so the commands are
                // registered by the time the console accepts input.
                crate::features::tokio_runtime::TOKIO_RUNTIME.spawn(async move {
                    let _ = crate::features::events::initialize_client_commands().await;
                });
            }
            spawn_cli_handler();
            if cli_args().is_server() {
                world_init();
            }
        });
    }
}

#[cfg(feature="server_registration")]
fn start_server_registration(all_mods: Vec<features::Mod>, active_mods: Vec<features::Mod>) {
    let args = cli_args();
    if !args.is_server() || !args.register {
        return;
    }
    let advertised_local_ip = if args.use_ipv6 && !args.local_ip_explicit {
        None
    } else {
        Some(args.local_ip.as_str())
    };
    let reg = Registration::new(
        (std::net::Ipv4Addr::LOCALHOST, args.game_server_query_port).into(),
        advertised_local_ip,
        all_mods,
        active_mods,
    );
    *globals().registration.lock().unwrap() = Some(reg);
    sinfo!("Started server registration manager");
}

pub fn world_init() {
    #[cfg(feature="rcon_commands")]
    thread::spawn(|| {
        handle_rcon();
    });

    let args = &cli_args();
    sinfo!("Server: {}, Discord: {}", args.is_server(), args.discord_enabled());

    #[cfg(all(feature="server_registration", not(feature="mod_management")))]
    start_server_registration(Vec::new(), Vec::new());

    // Mod manager requires world
    while globals().world().is_none() {
        thread::sleep(Duration::from_millis(500));
    }

    {
        TOKIO_RUNTIME.spawn(async move {
            let _ = features::events::initialize_subscribers().await;

            EVENT_SYSTEM.game_event_publisher.publish(GameChatMessageEvent(GameChatMessage {
                chat_source: ChatSource::Console(ChatType::Global),
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
                    chat_source: ChatSource::Console(ChatType::Global),
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
        drop(global_mm);

        // Server fallback: register every mod asset into the Asset Registry once.
        // The primary trigger is LoadFrontEndMap (fires after the paks are
        // mounted, on both client and server); this covers the case where a
        // dedicated server never loads the frontend. The sync registration must
        // run on the game thread, and the helper's one-shot guard makes it
        // idempotent regardless of which site fires first.
        // Jobs run in queue order, so this is queued before the mod scan below:
        // the scan must see the markers this registers.
        use crate::unchained::patches::unchained_integration::game_thread::run_on_game_thread;
        run_on_game_thread(|| {
            crate::features::mod_maps::auto_register_all_assets();
        });

        run_on_game_thread(move || {
            sinfo!("Mod scans in progress");
            mm.scan_asset_registry();
            mm.update_save_game();
            sinfo!("Started mod manager");
            #[cfg(feature="server_registration")]
            start_server_registration(
                mm.get_available().into_values().collect(),
                mm.get_active_mod_metadata(),
            );
        });
    }
}

/// The base program address of the running application will be used.
pub fn attach_hooks(snapshot: &SelectorSnapshot) -> Result<(), Box<dyn std::error::Error>> {
    attach_hooks_for::<unchained_macros_patternsleuth::PatternsleuthResolver>(snapshot)
}

fn attach_hooks_for<R: unchained_macros_core::AddressResolver>(snapshot: &SelectorSnapshot) -> Result<(), Box<dyn std::error::Error>> {
    sdebug!("Attaching hooks via auto-discovery:");
    unchained_macros_core::validate_listener_registrations_for::<R>(snapshot)?;

    for hook in unchained_macros_core::hook_inventory_for::<R>() {
        if !hook.resolver().supports(snapshot) || hook.initialized() {
            continue;
        }
        match unsafe { hook.attach(true) } {
            Ok(_) => sinfo!("☑ {} {}", hook.name, if hook.enabled() { "attached" } else { "attached (passive)" }),
            Err(e) => {
                serror!("☐ {}: {}", hook.name.to_uppercase(), e);
                if unchained_macros_core::isolate_observer(hook.condition).unwrap_or(false) {
                    return Err(e);
                }
            },
        }
    }

    for listener in unchained_macros_core::listener_inventory_for::<R>() {
        if !listener.resolver.supports(snapshot) || listener.initialized() { continue; }
        match unsafe { listener.attach(true) } {
            Ok(()) => sinfo!("☑ {} listener {}", listener.name, if listener.enabled() { "attached" } else { "attached (passive)" }),
            Err(error) => {
                serror!("☐ {} listener: {}", listener.name, error);
                if unchained_macros_core::isolate_observer(listener.condition).unwrap_or(false) { return Err(error); }
            },
        }
    }

    Ok(())
}

struct NativePatchWriter;

impl unchained_macros_core::PatchWriter for NativePatchWriter {
    unsafe fn bytes(&mut self, address: u64, bytes: &[u8]) -> Result<(), String> {
        unsafe { tools::memtools::patch(address as *mut u8, bytes); }
        Ok(())
    }
    unsafe fn nop(&mut self, address: u64, count: usize) -> Result<(), String> {
        unsafe { tools::memtools::nop(address as *mut u8, count); }
        Ok(())
    }
    unsafe fn write(&mut self, address: u64, value: &unchained_macros_core::WriteValue) -> Result<(), String> {
        unsafe { tools::memtools::write_raw(address as *mut u8, value.as_ptr(), value.size()); }
        Ok(())
    }
}

/// Apply named patches through the existing native memory-protection backend.
unsafe fn apply_patches<R: unchained_macros_core::AddressResolver>(context: &R::Context, snapshot: &SelectorSnapshot, state: &mut unchained_macros_core::PatchState) -> Result<(), String> {
    let results = unsafe { unchained_macros_core::apply_patches_for::<R, _>(
        context, snapshot, &mut NativePatchWriter, state,
    ) }?;
    for result in results {
        if result.applied {
            sinfo!("[+] Patch Applied: {} ({})", result.name, result.resolver);
        } else {
            sdebug!("[.] Patch Skipped (Condition not met): {} ({})", result.name, result.resolver);
        }
    }
    Ok(())
}
