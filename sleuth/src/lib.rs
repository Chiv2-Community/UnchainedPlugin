

mod resolvers;
mod scan;

use std::env;
use std::fs::File;
use std::io::{BufReader, Read};
use std::io::{BufWriter, Write};
use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use serde::Serialize;
use serde_json::to_writer_pretty;
use self::resolvers::{PLATFORM, PlatformType};

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
    offsets: HashMap<String, u64>,
}

fn expand_env_path(path: &str) -> Option<PathBuf> {
    if let Some(stripped) = path.strip_prefix("%LOCALAPPDATA%") {
        if let Ok(base) = env::var("LOCALAPPDATA") {
            return Some(PathBuf::from(base).join(stripped.trim_start_matches(['\\', '/'])));
        }
    }
    None
}

pub fn dump_builds(offsets: HashMap<String, u64>) -> Result<()> {
    let builds_path = expand_env_path(r"%LOCALAPPDATA%\Chivalry 2\Saved\Config\c2uc.builds.json").unwrap().to_path_buf();
    println!("JSON PATH {}", builds_path.to_string_lossy());
    let file = File::create(builds_path)?;
    let mut writer = BufWriter::new(file);
    let mut data = HashMap::new();

    let mut file_path = String::new();

    match env::current_exe() {
        Ok(path) => file_path = path.to_string_lossy().into(),
        Err(e) => eprintln!("Failed to get path: {}", e),
    }
    println!("Current executable path: {:?}", file_path);
    
    let crc32 = unsafe { crc32_from_file(&file_path) }.expect("Failed to compute CRC");


    let build_info = BuildInfo {
        build: 0,
        file_hash: crc32,
        name: "".to_string(),
        platform: PLATFORM.get().ok_or("OTHER").unwrap().to_string(),//platform.to_string(),
        path: file_path.to_string(),
        offsets,
    };

    data.insert(crc32.to_string(), build_info);
    to_writer_pretty(&mut writer, &data)?;
    writer.flush()?;

    Ok(())
}

#[no_mangle]
pub extern "C" fn generate_json() -> u8 {    
    let platform = match env::args().any(|arg| arg == "-epicapp=Peppermint") {
        true => PlatformType::EGS,
        false => PlatformType::STEAM
    };

    PLATFORM.set(platform).expect("Platform already set");

    let offsets = scan::scan().expect("Failed to scan");
    let len_u8 = offsets.len() as u8;
    dump_builds(offsets).expect("Failed to dump builds JSON");
    len_u8
}
