

mod resolvers;
mod scan;

use std::env;
use std::fs::File;
use std::io::{BufReader, Read};
use std::io::{BufWriter, Write};
use std::collections::HashMap;
use std::path::PathBuf;
use std::ffi::CString;
use std::os::raw::c_char;
use std::sync::Mutex;
use once_cell::sync::Lazy;

use anyhow::Result;
use serde::{Serialize, Deserialize};
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

        
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all(serialize = "PascalCase", deserialize = "snake_case"))]
struct BuildInfo {
    build: u32,
    file_hash: u32,
    name: String,
    platform: String,
    path: String,
    offsets: HashMap<String, u64>,
}

static CURRENT_BUILD_INFO: Lazy<Mutex<Option<BuildInfo>>> = Lazy::new(|| Mutex::new(None));
static KNOWN_BUILDS: Lazy<Mutex<HashMap<String, BuildInfo>>> = Lazy::new(|| Mutex::new(HashMap::new()));

fn expand_env_path(path: &str) -> Option<PathBuf> {
    if let Some(stripped) = path.strip_prefix("%LOCALAPPDATA%") {
        if let Ok(base) = env::var("LOCALAPPDATA") {
            return Some(PathBuf::from(base).join(stripped.trim_start_matches(['\\', '/'])));
        }
    }
    None
}

pub fn load_builds() -> Result<()> {
    let builds_path = expand_env_path(r"%LOCALAPPDATA%\Chivalry 2\Saved\Config\c2uc.builds.json").unwrap().to_path_buf();
    if builds_path.exists() {
        let file = File::open(&builds_path)?;
        let reader = BufReader::new(file);
        let data: HashMap<String, BuildInfo> = serde_json::from_reader(reader).unwrap_or_default();
        let mut known = KNOWN_BUILDS.lock().unwrap();
        *known = data;
    }
    Ok(())
}

pub fn dump_builds() -> Result<()> {
    let builds_path = expand_env_path(r"%LOCALAPPDATA%\Chivalry 2\Saved\Config\c2uc.builds.json").unwrap().to_path_buf();
    println!("JSON PATH {}", builds_path.to_string_lossy());
    
    let known = KNOWN_BUILDS.lock().unwrap();
    let file = File::create(builds_path)?;
    let mut writer = BufWriter::new(file);
    to_writer_pretty(&mut writer, &*known)?;
    writer.flush()?;

    Ok(())
}

#[no_mangle]
pub extern "C" fn scan_build() -> u8 {
    println!("Scanning build...");
    
    let platform = match env::args().any(|arg| arg == "-epicapp=Peppermint") {
        true => PlatformType::EGS,
        false => PlatformType::STEAM
    };

    PLATFORM.set(platform).ok(); // Ignore if already set

    let offsets = scan::scan().expect("Failed to scan");
    
    let mut file_path = String::new();
    match env::current_exe() {
        Ok(path) => file_path = path.to_string_lossy().into(),
        Err(e) => eprintln!("Failed to get path: {}", e),
    }
    
    let crc32 = unsafe { crc32_from_file(&file_path) }.expect("Failed to compute CRC");

    let build_info = BuildInfo {
        build: 0,
        file_hash: crc32,
        name: "".to_string(),
        platform: PLATFORM.get().unwrap_or(&PlatformType::OTHER).to_string(),
        path: file_path.to_string(),
        offsets,
    };

    let len = build_info.offsets.len() as u8;

    // Save to CURRENT_BUILD_INFO
    {
        let mut current = CURRENT_BUILD_INFO.lock().unwrap();
        *current = Some(build_info.clone());
    }

    // Update KNOWN_BUILDS
    {
        let mut known = KNOWN_BUILDS.lock().unwrap();
        known.insert(crc32.to_string(), build_info);
    }

    // Still dump to JSON for future use
    if let Err(e) = dump_builds() {
        eprintln!("Failed to dump builds JSON: {}", e);
    }

    len
}

#[no_mangle]
pub extern "C" fn load_known_builds() {
    if let Err(e) = load_builds() {
        eprintln!("Failed to load known builds: {}", e);
    }
}

#[no_mangle]
pub extern "C" fn get_known_builds_count() -> usize {
    let known = KNOWN_BUILDS.lock().unwrap();
    known.len()
}

#[no_mangle]
pub extern "C" fn get_known_build_hash(index: usize) -> u32 {
    let known = KNOWN_BUILDS.lock().unwrap();
    known.values().nth(index).map(|bi| bi.file_hash).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn get_known_build_platform(index: usize) -> *mut c_char {
    let known = KNOWN_BUILDS.lock().unwrap();
    let platform = known.values().nth(index).map(|bi| bi.platform.as_str()).unwrap_or("OTHER");
    CString::new(platform).unwrap().into_raw()
}

#[no_mangle]
pub extern "C" fn get_known_build_offset_count(index: usize) -> usize {
    let known = KNOWN_BUILDS.lock().unwrap();
    known.values().nth(index).map(|bi| bi.offsets.len()).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn get_known_build_offset_name(build_index: usize, offset_index: usize) -> *mut c_char {
    let known = KNOWN_BUILDS.lock().unwrap();
    if let Some(bi) = known.values().nth(build_index) {
        if let Some((name, _)) = bi.offsets.iter().nth(offset_index) {
            return CString::new(name.as_str()).unwrap().into_raw();
        }
    }
    std::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn get_known_build_offset_value(build_index: usize, offset_index: usize) -> u64 {
    let known = KNOWN_BUILDS.lock().unwrap();
    if let Some(bi) = known.values().nth(build_index) {
        if let Some((_, value)) = bi.offsets.iter().nth(offset_index) {
            return *value;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn get_file_hash() -> u32 {
    let current = CURRENT_BUILD_INFO.lock().unwrap();
    current.as_ref().map(|bi| bi.file_hash).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn get_platform() -> *mut c_char {
    let current = CURRENT_BUILD_INFO.lock().unwrap();
    let platform = current.as_ref().map(|bi| bi.platform.as_str()).unwrap_or("OTHER");
    CString::new(platform).unwrap().into_raw()
}

#[no_mangle]
pub extern "C" fn get_offset_count() -> usize {
    let current = CURRENT_BUILD_INFO.lock().unwrap();
    current.as_ref().map(|bi| bi.offsets.len()).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn get_offset_name(index: usize) -> *mut c_char {
    let current = CURRENT_BUILD_INFO.lock().unwrap();
    if let Some(bi) = current.as_ref() {
        if let Some((name, _)) = bi.offsets.iter().nth(index) {
            return CString::new(name.as_str()).unwrap().into_raw();
        }
    }
    std::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn get_offset_value(index: usize) -> u64 {
    let current = CURRENT_BUILD_INFO.lock().unwrap();
    if let Some(bi) = current.as_ref() {
        if let Some((_, value)) = bi.offsets.iter().nth(index) {
            return *value;
        }
    }
    0
}

#[no_mangle]
pub extern "C" fn free_string(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(s));
    }
}

#[no_mangle]
pub extern "C" fn generate_json() -> u8 {
    scan_build()
}
