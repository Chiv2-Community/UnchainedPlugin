use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::builder::{
    IntoResettable, PossibleValue, PossibleValuesParser, TypedValueParser, ValueParser,
};
use indicatif::ProgressBar;
use patternsleuth::resolvers::{resolvers, NamedResolver};

use std::fs::File;
use std::io::{BufReader, Read};
use std::io::{BufWriter, Write};

use serde::Serialize;
use serde_json::to_writer_pretty;

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

fn parse_maybe_hex(s: &str) -> Result<usize> {
    Ok(s.strip_prefix("0x")
        .map(|s| usize::from_str_radix(s, 16))
        .unwrap_or_else(|| s.parse())?)
}

fn resolver_parser() -> impl IntoResettable<ValueParser> {
    fn parse_resolver(s: &str) -> Result<&'static NamedResolver> {
        resolvers()
            .find(|res| s == res.name)
            .context("Resolver not found")
    }
    fn possible_resolvers() -> Vec<PossibleValue> {
        resolvers().map(|r| r.name.into()).collect()
    }
    PossibleValuesParser::new(possible_resolvers()).map(|v| parse_resolver(&v).unwrap())
}

// TODO remove, only used for patterns/xrefs from CLI
#[derive(Debug, Clone, Eq, Hash, PartialEq)]
struct Sig(String);
impl std::fmt::Display for Sig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(&self, f)
    }
} 
use std::process;
use std::env;
pub fn scan() -> Result<()> {
    let game = "";
    let pid = Some(process::id() as i32);

    let resolvers = resolvers().collect::<Vec<_>>();
    let dyn_resolvers = resolvers.iter().map(|res| res.getter).collect::<Vec<_>>();

    let mut games: HashSet<String> = Default::default();
    

    enum Output {
        Stdout,
        Progress(ProgressBar),
    }

    impl Output {
        fn println<M: AsRef<str>>(&self, msg: M) {
            match self {
                Output::Stdout => println!("{}", msg.as_ref()),
                Output::Progress(progress) => progress.println(msg),
            }
        }
    }

    let mut games_vec = vec![];

    if let Some(pid) = pid {
        games_vec.push(GameEntry::Process(GameProcessEntry { pid }));
    } 
    
    let output = Output::Stdout;
    let iter = Box::new(games_vec.iter());

    let file = File::create("out.json")?;
    // let writer = BufWriter::new(file);
    let mut writer = BufWriter::new(file);
    let mut data = HashMap::new();
    
    for game in iter {
        #[allow(unused_assignments)]
        // let mut bin_data = None;
        let mut file_path = String::new();

        let name = format!("PID={}", pid.unwrap());
        let exe = patternsleuth::process::external::read_image_from_pid(pid.unwrap())?;
        println!("GAME LNC '{}' '{}'", name, exe.base_address);

        games.insert(name.to_string());

        // let scan = exe.scan(&patterns)?;

        let game_name = match game {
            GameEntry::File(GameFileEntry { name, .. }) => name.clone(),
            GameEntry::Process(GameProcessEntry { pid }) => format!("pid={pid}"),
        };

        let resolution = tracing::info_span!("scan", game = game_name)
            .in_scope(|| exe.resolve_many(&dyn_resolvers));

        
        #[derive(Serialize)]
        struct BuildInfo {
            Build: u32,
            FileHash: u32,
            Name: String,
            Platform: String,
            Path: String,
            Offsets: HashMap<String, u64>,
        }
        // let crc32: u32 = 1937620090;   
        // println!("Path: {}", file_path);
        let mut crc32: u32 = 0;
        if file_path.is_empty() {
            match env::current_exe() {
                Ok(path) => file_path = path.to_string_lossy().into(),
                Err(e) => eprintln!("Failed to get path: {}", e),
            }
            println!("Current executable path: {:?}", file_path)
        }
        
        crc32 = unsafe { crc32_from_file(&file_path) }.expect("Failed to compute CRC");
        
        // let crc32 = crc32_from_file(&file_path).unwrap();

        
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
                    let val = u64::from_str_radix(hex.trim_start_matches("0x"), 16)? & 0xFFFFFFF;
                    println!("{} {} {}", resolver.name, hex, val);
                    offsets.insert(resolver.name.to_string(), val);
                }
            }
        } 

        let build_info = BuildInfo {
            Build: 0,
            FileHash: crc32,
            Name: "".to_string(),
            Platform: game_name.to_uppercase().to_string(),
            Path: file_path.to_string(),
            Offsets: offsets,
        };

        data.insert(crc32.to_string(), build_info);
    }
    
    to_writer_pretty(&mut writer, &data)?;
    writer.flush()?;

    Ok(())
}

enum GameEntry {
    File(GameFileEntry),
    Process(GameProcessEntry),
}

struct GameFileEntry {
    name: String,
    exe_path: PathBuf,
}

struct GameProcessEntry {
    pid: i32,
}