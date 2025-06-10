

mod resolvers;
mod scan;
use std::time::Duration;
use std::{env, thread};
use std::fs::File;
use std::io::{BufReader, Read};
use std::io::{BufWriter, Write};
use std::{collections::HashMap, path::PathBuf};
mod ue;
mod tools;
mod chiv2;

use anyhow::Result;
use patternsleuth::resolvers::unreal::blueprint_library::UFunctionBind;
use patternsleuth::resolvers::unreal::*;
// use dll_hook::ue::*;
use patternsleuth::resolvers::unreal::game_loop::FEngineLoopInit;
use patternsleuth::resolvers::unreal::kismet::{FFrameStep, FFrameStepExplicitProperty, FFrameStepViaExec};
use patternsleuth::resolvers::unreal::KismetSystemLibrary;
use patternsleuth::resolvers::unreal::{fname::FNameToString,
                                game_loop::UGameEngineTick,
                                gmalloc::GMalloc,
                                guobject_array::{FUObjectArrayAllocateUObjectIndex, FUObjectArrayFreeUObjectIndex, GUObjectArray}
                            };
use resolvers::admin_control::*;
#[cfg(feature="kismet-log")]
use resolvers::kismet_dev::*;
use serde::Serialize;
use serde_json::to_writer_pretty;
use tools::logger::init_syslog;
#[cfg(feature="server-registration")]
use tools::server_registration::Registration;
use self::resolvers::{PLATFORM, BASE_ADDR, PlatformType};

use log::info;
use clap::{command, CommandFactory, Parser, Subcommand};

pub static TEST_INTRO: &str = "\x1b[38;5;228m\
\
▄████████    ▄█    █▄     ▄█   ▄█    █▄     ▄████████  ▄█          ▄████████ ▄██   ▄         ▄█   ▄█ \r\n\
███    ███   ███    ███   ███  ███    ███   ███    ███ ███         ███    ███ ███   ██▄      ███  ███ \r\n\
███    █▀    ███    ███   ███▌ ███    ███   ███    ███ ███         ███    ███ ███▄▄▄███      ███▌ ███▌\r\n\
███         ▄███▄▄▄▄███▄▄ ███▌ ███    ███   ███    ███ ███        ▄███▄▄▄▄██▀ ▀▀▀▀▀▀███      ███▌ ███▌\r\n\
███        ▀▀███▀▀▀▀███▀  ███▌ ███    ███ ▀███████████ ███       ▀▀███▀▀▀▀▀   ▄██   ███      ███▌ ███▌\r\n\
███    █▄    ███    ███   ███  ███    ███   ███    ███ ███       ▀███████████ ███   ███      ███  ███ \r\n\
███    ███   ███    ███   ███  ███    ███   ███    ███ ███▌    ▄   ███    ███ ███   ███      ███  ███ \r\n\
████████▀    ███    █▀    █▀    ▀██████▀    ███    █▀  █████▄▄██   ███    ███  ▀█████▀       █▀   █▀  \r\n\
                                                       ▀           ███    ███                         \r\n\
\x1b[38;5;1m███    █▄  ███▄▄▄▄    ▄████████    ▄█    █▄       ▄████████  ▄█  ███▄▄▄▄      ▄████████ ████████▄     \r\n\
███    ███ ███▀▀▀██▄ ███    ███   ███    ███     ███    ███ ███  ███▀▀▀██▄   ███    ███ ███   ▀███    \r\n\
███    ███ ███   ███ ███    █▀    ███    ███     ███    ███ ███▌ ███   ███   ███    █▀  ███    ███    \r\n\
███    ███ ███   ███ ███         ▄███▄▄▄▄███▄▄   ███    ███ ███▌ ███   ███  ▄███▄▄▄     ███    ███    \r\n\
███    ███ ███   ███ ███        ▀▀███▀▀▀▀███▀  ▀███████████ ███▌ ███   ███ ▀▀███▀▀▀     ███    ███    \r\n\
███    ███ ███   ███ ███    █▄    ███    ███     ███    ███ ███  ███   ███   ███    █▄  ███    ███    \r\n\
███    ███ ███   ███ ███    ███   ███    ███     ███    ███ ███  ███   ███   ███    ███ ███   ▄███    \r\n\
████████▀   ▀█   █▀  ████████▀    ███    █▀      ███    █▀  █▀    ▀█   █▀    ██████████ ████████▀     \r\n\
\x1b[38;5;255m                                                                                                      \r\n\
                                                                                                      ";

pub static TEST_INTRO3: &str = "\
|            █               \r\n\
|          ███████           \r\n\
|          ██  ███           \r\n\
|      ███   █████    ██     \r\n\
|     █████     █   █████    \r\n\
|    ████████       ███████  \r\n\
|    ███████████    ███   ██ \r\n\
|    █████████████   ███  ███\r\n\
|     ██      ███      █████ \r\n\
|     ██      ████      █    \r\n\
|    ███    ████████   ██    \r\n\
|    ██████████  █████████   \r\n\
|       ██████   ████████    \r\n\
|        █████  █████        \r\n\
|         ███████████        \r\n\
|         ███████ ██         \r\n\
|           ██ █   █         \r\n\
";

// pub static test: &str = "\x1b[38;5;196m▄\x1b[0m\x1b[38;5;202m█\x1b[0m\x1b[38;5;226m█\x1b[0m\x1b[38;5;46m█\x1b[0m\x1b[38;5;21m█\x1b[0m\x1b[38;5;93m█\x1b[0m\x1b[38;5;201m█\x1b[0m\x1b[38;5;196m█\x1b[0m\x1b[38;5;202m█\x1b[0m    \x1b[38;5;226m▄\x1b[0m\x1b[38;5;46m█\x1b[0m    \x1b[38;5;21m█\x1b[0m\x1b[38;5;93m▄\x1b[0m     \x1b[38;5;201m▄\x1b[0m\x1b[38;5;196m█\x1b[0m   \x1b[38;5;202m▄\x1b[0m\x1b[38;5;226m█\x1b[0m    \x1b[38;5;46m█\x1b[0m\x1b[38;5;21m▄\x1b[0m     \x1b[38;5;93m▄\x1b[0m\x1b[38;5;201m█\x1b[0m\x1b[38;5;196m█\x1b[0m\x1b[38;5;202m█\x1b[0m\x1b[38;5;226m█\x1b[0m\x1b[38;5;46m█\x1b[0m\x1b[38;5;21m█\x1b[0m\x1b[38;5;93m█\x1b[0m\x1b[38;5;201m█\x1b[0m  \x1b[38;5;196m▄\x1b[0m\x1b[38;5;202m█\x1b[0m          \x1b[38;5;226m▄\x1b[0m\x1b[38;5;46m█\x1b[0m\x1b[38;5;21m█\x1b[0m\x1b[38;5;93m█\x1b[0m\x1b[38;5;201m█\x1b[0m\x1b[38;5;196m█\x1b[0m\x1b[38;5;202m█\x1b[0m\x1b[38;5;226m█\x1b[0m\x1b[38;5;46m█\x1b[0m \x1b[38;5;21m▄\x1b[0m\x1b[38;5;93m█\x1b[0m\x1b[38;5;201m█\x1b[0m   \x1b[38;5;196m▄\x1b[0m         \x1b[38;5;202m▄\x1b[0m\x1b[38;5;226m█\x1b[0m   \x1b[38;5;46m▄\x1b[0m\x1b[38;5;21m█\x1b[0m\n\x1b[38;5;93m█\x1b[0m\x1b[38;5;201m█\x1b[0m\x1b[38;5;196m█\x1b[0m    \x1b[38;5;202m█\x1b[0m\x1b[38;5;226m█\x1b[0m\x1b[38;5;46m█\x1b[0m   \x1b[38;5;21m█\x1b[0m\x1b[38;5;93m█\x1b[0m\x1b[38;5;201m█\x1b[0m    \x1b[38;5;196m█\x1b[0m\x1b[38;5;202m█\x1b[0m\x1b[38;5;226m█\x1b[0m   \x1b[38;5;46m█\x1b";


// CLI args
#[derive(Debug, Subcommand)]
enum Commands {
    #[allow(clippy::upper_case_acronyms)]
    TBL,
    #[allow(clippy::upper_case_acronyms)]
    NONE,
}
// #[derive(Parser, Debug)]
// #[command(name = "Chivalry 2 Unchained", version = "1.0", author = "Unchained Team")]
#[derive(Parser, Debug)]
#[command(name = "Chivalry 2 Unchained", author = "Unchained Team", version, about, long_about = None)]
// FIXME: possible to skip unhandled args?
struct CLIArgs {    
    // #[arg()]
    // positional_args: Vec<String>,
    // #[arg()]
    // game_id: String,

    #[arg(long = "next-map-mod-actors")]
    next_mod_actors: Option<Vec<String>>,

    #[arg(long = "all-mod-actors")]
    mod_paks: Option<Vec<String>>,

    #[arg(long = "unchained")]
    is_unchained: bool,
    // 
    #[arg(long = "rcon")]
    rcon_port: Option<u16>,
    // // 
    #[arg(long = "desync-patch")]
    apply_desync_patch: bool,
    // 
    #[arg(long = "use-backend-banlist")]
    use_backend_banlist: bool,
    // 
    #[arg(long = "nullrhi")]
    is_headless: bool,
    // 
    #[arg(long = "next-map-name")]
    next_map: Option<String>,
    // 
    #[arg(long = "playable-listen")]
    playable_listen: bool,
    // // 
    #[arg(long = "server-browser-backend")]
    server_browser_backend: Option<String>,
    // // 
    #[arg(long = "server-password")]
    server_password: Option<String>,
    // s
    #[arg(long = "platform")]
    platform: Option<String>,

    #[arg(long = "GameServerPingPort", default_value="3075")]
    game_server_ping_port: Option<u16>,

    #[arg(long = "GameServerQueryPort", default_value="7071")]
    game_server_query_port: Option<u16>,

    #[arg(long = "Port", default_value="7777")]
    game_port: Option<u16>,


    // UNHANDLED START
    #[arg(long = "AUTH_LOGIN")]
    auth_login: Option<String>,
    #[arg(long = "AUTH_PASSWORD")]
    auth_password: Option<String>,
    #[arg(long = "AUTH_TYPE")]
    auth_type: Option<String>,
    #[arg(long = "epicapp")]
    epicapp: Option<String>,
    #[arg(long = "epicenv")]
    epicenv: Option<String>,
    #[arg(long = "EpicPortal")]
    epic_portal: bool,
    #[arg(long = "epicusername")]
    epicusername: Option<String>,
    #[arg(long = "epicuserid")]
    epicuserid: Option<String>,
    #[arg(long = "epiclocale")]
    epiclocale: Option<String>,
    #[arg(long = "epicsandboxid")]
    epicsandboxid: Option<String>,
    // UNHANDLED END

    #[arg(trailing_var_arg = true)]
    pub extra_args: Vec<String>,
}

// IEEE
use std::arch::x86_64::_mm_crc32_u8;
#[target_feature(enable = "sse4.2")]
unsafe fn crc32_from_file(path: &str) -> std::io::Result<u32> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut buffer = [0u8; 4096];
    let mut crc: u32 = 0;

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }

        for &byte in &buffer[..bytes_read] {
            crc = _mm_crc32_u8(crc, byte);
        }
    }

    Ok(crc ^ 0xFFFFFFFF)
}

        
#[derive(Serialize)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "snake_case"))]
struct BuildInfo {
    build: u32,
    file_hash: u32,
    name: String,
    platform: String,
    path: String,
    offsets: HashMap<String, usize>,
}

fn expand_env_path(path: &str) -> Option<PathBuf> {
    if let Some(stripped) = path.strip_prefix("%LOCALAPPDATA%") {
        if let Ok(base) = env::var("LOCALAPPDATA") {
            return Some(PathBuf::from(base).join(stripped.trim_start_matches(['\\', '/'])));
        }
    }
    None
}

pub fn dump_builds() -> Result<()> {
    let builds_path = expand_env_path(r"%LOCALAPPDATA%\Chivalry 2\Saved\Config\c2uc.builds.json").unwrap().to_path_buf();
    // println!("JSON PATH {}", builds_path.to_string_lossy());
    let file = File::create(builds_path)?;
    let mut writer = BufWriter::new(file);
    let mut data = HashMap::new();

    // let mut file_path = String::new();

    let offsets = crate::scan::OFFSETS.get().unwrap();
    let base_addr = BASE_ADDR.get().unwrap();

    let file_path: String = env::current_exe().unwrap().to_string_lossy().into();

    // match env::current_exe() {
    //     Ok(path) => file_path = path.to_string_lossy().into(),
    //     Err(e) => eprintln!("Failed to get path: {}", e),
    // }
    // println!("Current executable path: {:?}", file_path);
    
    let crc32 = unsafe { crc32_from_file(&file_path) }.expect("Failed to compute CRC");

    let base_offsets: HashMap<String, usize> = offsets
                                                .iter()
                                                .map(|(k, v)| (k.clone(), v - base_addr))
                                                .collect();

    let build_info = BuildInfo {
        build: 0,
        file_hash: crc32,
        name: "".to_string(),
        platform: PLATFORM.get().ok_or("OTHER").unwrap().to_string(),//platform.to_string(),
        path: file_path.to_string(),
        offsets: base_offsets,
    };

    data.insert(crc32.to_string(), build_info);
    to_writer_pretty(&mut writer, &data)?;
    writer.flush()?;

    Ok(())
}





pub unsafe fn attach_hooks(base_address: usize, offsets: HashMap<String, u64>) -> Result<(), Box<dyn std::error::Error>> {

    // attach_GameEngineTick(base_address, offsets).unwrap();
    info!("Attaching hooks:");
    
    let hooks_new = attach_hooks_list![[
        UGameEngineTick,
        ExecuteConsoleCommand,
        FEngineLoopInit,
        ClientMessage,
        #[cfg(feature="demo")]
        SomeRandomFunction,
        // StaticFindObjectSafe,
        #[cfg(feature="kismet-log")]
        KismetExecutionMessage,
        #[cfg(feature="dev")]
        LogReliableRPC,
        #[cfg(feature="dev")]
        LogReliableRPCFailed,
    ]];
    
    // use crate::resolvers::macros;
    hooks_new.iter().for_each(|(s, f)| {
        match (f)(base_address, offsets.clone()) {
            Ok(_) => {
                sinfo![f; "☑ {} ", s]
            },
            Err(e) => {
                // sdebug!(file, f;"☐ {}: {}", s.to_uppercase(), e);
                // strace!(file, f, line;"☐ {}: {}", s.to_uppercase(), e);
                // swarn!(file, f, line;"☐ {}: {}", s.to_uppercase(), e);
                // sinfo!(file, func, line, mod;"☐ {}: {}", s.to_uppercase(), e);
                // serror!(file, func, line, column;"☐ {}: {}", s.to_uppercase(), e);
                // debug_where!();
// console -> [23:54:45 ERROR] [attach_hooks] ☐ SOMERANDOMFUNCTION: No address found.
// file -> [2025-06-09 23:54:45 13116 ERROR | function ] [attach_hooks] ☐ SOMERANDOMFUNCTION: No address found.

// console -> [23:54:45 ERROR] [src\lib.rs|sleuthlib::attach_hooks::{{closure}}|L317|C17] ☐ SOMERANDOMFUNCTION: No address found.
// file -> [2025-06-09 23:54:45 13116 ERROR | function ] [src\lib.rs|sleuthlib::attach_hooks::{{closure}}|L317|C17] ☐ SOMERANDOMFUNCTION: No address found.

                // error!("☐ {}: {}", s.to_uppercase(), e);
                serror!(f; "☐ {}: {}", s.to_uppercase(), e);
                // serror!(file, func, line, column;"☐ {}: {}", s.to_uppercase(), e);
            },
        }
    });
    
    // resolvers::admin_control::attach_UGameEngineTick(base_address, offsets.clone()).unwrap();
    // resolvers::admin_control::attach_ExecuteConsoleCommand(base_address, offsets.clone()).unwrap();
    // resolvers::admin_control::attach_FEngineLoopInit(base_address, offsets.clone()).unwrap();
    // resolvers::admin_control::attach_ClientMessage(base_address, offsets.clone()).unwrap();
    Ok(())
  }

// We're using a mix of cli arg types, normalize them to --key value(s)
// e.g. -rcon 9001, -epicsomething=blablabla, Port=7777
// This function converts all of those to --convention. It also drops game_identifier
// To parse it all with clap, it checks against entries in CLIArgs and filters out unhandled ones
fn normalize_and_filter_args<I: IntoIterator<Item = String>>(args: I) -> Vec<String> {
    let mut args = args.into_iter();
    let bin_name = args.next().unwrap_or_else(|| "app".to_string());

    let known_flags: Vec<String> = CLIArgs::command()
        .get_arguments()
        .filter_map(|a| a.get_long().map(|s| format!("--{s}")))
        .collect();

    let mut result = vec![bin_name];
    let mut args = args.peekable();
    let mut last_flag: Option<String> = None;
    let mut last_opt: Option<String> = None;
    

    while let Some(arg) = args.next() {
        // println!("-- LINE: {arg}");
        // Normalize `key=value` and `-flag` → `--flag`
        let (flag, value_opt): (String, Option<String>) = 
            if let Some((k, v)) = arg.split_once('=') {
                (format!("--{}", k.trim_start_matches('-')), Some(v.to_string()))
            } else if arg.starts_with('-') && !arg.starts_with("--") && arg.len() > 2 {
                (format!("--{}", &arg[1..]), None)
            } else {
                (arg.clone(), None)
            };
        
        // println!("cur: {flag}");
        let cur_flag = flag.clone();
        if known_flags.contains(&flag) {
            result.push(flag);
            if let Some(v) = value_opt {
                // println!("option: {v}");
                last_opt = Some(v.clone());
                result.push(v);
            } 
            else if let Some(peek) = args.peek() {
                if !peek.starts_with('-') {
                    let var = args.next().unwrap();
                    // print!("pushing {var}");
                    result.push(var);
                }
            }
        }
        // args can split an option (e.g. --name Not Sure)
        else if !result.is_empty() && !flag.starts_with('-') { 
            let last_valid = result.last().unwrap();
            if last_flag.is_some() {
                // println!("Last '{last}' last valid '{last_valid}'");
                if let Some(o) = &last_opt {
                    // println!("Last '{}' last valid {} last option '{}' equal: {}", last, last_valid, o, o == last_valid);
                    // println!("Res: {} Trailing string {}, last flag {}, last result {}",result.len(), flag, last, last_valid);
                    if o == last_valid {
                        if let Some(last_mut) = result.last_mut() {
                            last_mut.push(' ');
                            last_mut.push_str(&cur_flag);
                        }
                    }
                }
            } 
        }
        last_flag = Some(cur_flag);
    }

    result
}

unsafe fn load_cli() -> Result<CLIArgs, clap::error::Error> {
    let args = std::env::args();
    let parsed = normalize_and_filter_args(args);
    let cli = CLIArgs::try_parse_from(parsed).expect("Failed to parse CLI atgs");
    // println!("Parsed CLI: {:#?}", cli);
    Ok(cli)
}
unsafe fn init_globals() -> Result<(), clap::error::Error>{
    
    let platform = match env::args().any(|arg| arg == "-epicapp=Peppermint") {
        true => PlatformType::EGS,
        false => PlatformType::STEAM
    };
    
    let exe = patternsleuth::process::internal::read_image().map_err(|e| e.to_string()).expect("failed to read image");
    // FIXME: replace old references
    PLATFORM.set(platform).expect("Platform already set");
    BASE_ADDR.set(exe.base_address).expect("BASE_ADDR already set");
    // debug!("Platform: {} base_addr: '0x{:x?}'", platform, exe.base_address);

    // Load CLI ARGS
    let args = load_cli().expect("Failed to load CLI ARGS");
    let resolution = exe.resolve(DllHookResolution::resolver()).unwrap();
    
    // println!("results: {:?}", resolution);
    let guobject_array: &'static ue::FUObjectArray =
        &*(resolution.guobject_array.0 as *const ue::FUObjectArray);
        

    GLOBALS = Some(Globals {
        guobject_array: guobject_array.into(),
        resolution,
        main_thread_id: std::thread::current().id(),
        // last_command: None,
        base_address: exe.base_address,
        is_server: false,
        cli_args: args,
        platform
    });
    Ok(())
}



// █: 743
// ▀: 67
// ▄: 66
// r: 18
// n: 18
// ▌: 13
#[allow(dead_code)]
fn intro() {
    let mut color_index = 16;
    let max_color = 231;
    for line in TEST_INTRO.lines() {
        for ch in line.chars() {
            let color = format!("\x1b[38;5;{color_index}m");
            print!("{color}{ch}\x1b[0m");

            color_index += 1;
            if color_index > max_color {
                color_index = 16;
            }
            thread::sleep(Duration::from_micros(20));
        }
        println!(); // new line
    }
}

// https://stackoverflow.com/questions/38088067/equivalent-of-func-or-function-in-rust
// https://play.rust-lang.org/?version=stable&mode=debug&edition=2015&gist=df5975cd589ae7286a769e1c70e7715d
// #[allow(unused_macros)]
// macro_rules! function {
//     () => {{
//         fn f() {}
//         fn type_name_of<T>(_: T) -> &'static str {
//             std::any::type_name::<T>()
//         }
//         let name = type_name_of(f);
//         &name[..name.len() - 3]
//     }}
// }
// define_pocess!

#[no_mangle]
pub extern "C" fn generate_json() -> u8 {   
    // intro();
    // thread::sleep(Duration::from_secs(10));
    print!("{TEST_INTRO}");
    println!();
    init_syslog().expect("Failed to init syslog");
    unsafe { 
        match init_globals() {
            Ok(_) => {},
            Err(e) => serror!(f; "No globals: {}", e),
        }
     };

    #[cfg(feature="rcon")]
    std::thread::spawn(|| {
        resolvers::rcon::handle_rcon();
    });

    // (|| {
    //     mod module {
    //         pub trait Trait {
    //             fn function(&self) {
    //                 println!("{} (in {} [{}:{}:{}])",
    //                     function!(), module_path!(), file!(), line!(), column!()
    //                 );
    //             }
    //         }
    //         impl Trait for () {}
    //     }
    //     module::Trait::function(&());
    // })();
    
    // Init syslog    
    // info!("Info blabla");
    // warn!("Warning! ‼");
    // debug!("DEBUG MESSAGE");
    // error!("ERRROR ");

    // PLATFORM.set(platform).expect("Platform already set");
    let exe = patternsleuth::process::internal::read_image().map_err(|e| e.to_string()).expect("failed to read image");

    let offsets = scan::scan().expect("Failed to scan");
    let len_u8 = offsets.len() as u8;
    // FIXME: Nihi: ?
    unsafe {
        attach_hooks(exe.base_address, offsets).unwrap();
    }
    dump_builds().expect("Failed to dump builds JSON");
    
    #[cfg(feature="cli-commands")]
    std::thread::spawn(|| {
        resolvers::rcon::handle_cmd();
    });

    
    #[cfg(feature="server-registration")]
    {
        let cli = globals().args();
        
        let backend = cli.server_browser_backend.clone().unwrap();
        let reg = Registration::new(
            "127.0.0.1",
            7071
        );
        
        // let last_info = std::sync::Arc::clone(&tools::server_registration::REGISTRATION);
        // *last_info.lock().unwrap() = Some(reg);
        reg.start(&backend, "Chivalry 2 Local Server", "");
        let registration = std::sync::Arc::new(reg);
        registration.start_heartbeat(&backend, "Some Server", "");
        info!("Backend: {backend}");
        // std::thread::spawn(|| {
        // });

    }
    len_u8
}

patternsleuth::_impl_try_collector! {
    #[derive(Debug, PartialEq, Clone)]
    struct DllHookResolution {
        gmalloc: GMalloc,
        guobject_array: GUObjectArray,
        fnametostring: FNameToString,
        allocate_uobject: FUObjectArrayAllocateUObjectIndex,
        free_uobject: FUObjectArrayFreeUObjectIndex,
        game_tick: UGameEngineTick,
        engine_loop_init: FEngineLoopInit,
        kismet_system_library: KismetSystemLibrary,
        fframe_step_via_exec: FFrameStepViaExec,
        fframe_step: FFrameStep,
        fframe_step_explicit_property: FFrameStepExplicitProperty,
        // fframe_kismet_execution_message: FFrameKismetExecutionMessage,
        ufunction_bind: UFunctionBind,
        uobject_base_utility_get_path_name: UObjectBaseUtilityGetPathName,
    }
}


use serde::{Serializer, Deserialize, Deserializer};

// Stubs to disable unimplemented err
impl Serialize for DllHookResolution {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer, { todo!("Serialization not implemented") }
}
impl<'de> Deserialize<'de> for DllHookResolution {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where D: Deserializer<'de>, { todo!("Deserialization not implemented") }
}


// Globals impl from dll_hook example
// used by ue.rs

static mut GLOBALS: Option<Globals> = None;

pub struct Globals {
    resolution: DllHookResolution,
    guobject_array: parking_lot::FairMutex<&'static ue::FUObjectArray>,
    #[allow(dead_code)]
    main_thread_id: std::thread::ThreadId,
    // last_command: Option<ue::FString>,
    platform: PlatformType,
    base_address: usize,
    is_server: bool,
    cli_args: CLIArgs,
}

impl Globals {
    pub fn gmalloc(&self) -> &ue::FMalloc {
        unsafe { &**(self.resolution.gmalloc.0 as *const *const ue::FMalloc) }
    }
    pub fn fframe_step(&self) -> ue::FnFFrameStep {
        unsafe { std::mem::transmute(self.resolution.fframe_step.0) }
    }
    pub fn fframe_step_explicit_property(&self) -> ue::FnFFrameStepExplicitProperty {
        unsafe { std::mem::transmute(self.resolution.fframe_step_explicit_property.0) }
    }
    pub fn fname_to_string(&self) -> ue::FnFNameToString {
        unsafe { std::mem::transmute(self.resolution.fnametostring.0) }
    }
    pub fn uobject_base_utility_get_path_name(&self) -> ue::FnUObjectBaseUtilityGetPathName {
        unsafe { std::mem::transmute(self.resolution.uobject_base_utility_get_path_name.0) }
    }
    pub fn guobject_array(&self) -> parking_lot::FairMutexGuard<'static, &ue::FUObjectArray> {
        self.guobject_array.lock()
    }
    pub unsafe fn guobject_array_unchecked(&self) -> &ue::FUObjectArray {
        *self.guobject_array.data_ptr()
    }

    pub fn get_platform(&self) -> PlatformType {
        self.platform
    }

    pub fn get_base_address(&self) -> usize {
        self.base_address
    }

    pub fn is_server(&self) -> bool {
        self.is_server
    }

    fn args(&self) -> &CLIArgs {
        &self.cli_args
    }
}

// FIXME: Nihi: ?
#[allow(static_mut_refs)]
pub fn globals() -> &'static Globals {
    unsafe { GLOBALS.as_ref().unwrap() }
}