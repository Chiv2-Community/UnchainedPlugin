mod resolvers;
mod scan;

use std::env;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::collections::HashMap;
use std::path::PathBuf;
use std::os::raw::c_char;
use std::sync::Mutex;
use once_cell::sync::Lazy;

use anyhow::{Context, Result};
use serde::{Serialize, Deserialize};
use serde_json::to_writer_pretty;
use self::resolvers::{PLATFORM, PlatformType};

// IEEE
use std::arch::x86_64::{_mm_crc32_u8, _mm_crc32_u64};

#[target_feature(enable = "sse4.2")]
unsafe fn crc32_from_file(path: &str) -> std::io::Result<u32> {
    let file = File::open(path)?;
    let mmap = memmap2::Mmap::map(&file)?;
    let mut crc: u64 = 0;

    let mut chunks = mmap.chunks_exact(8);
    while let Some(chunk) = chunks.next() {
        crc = _mm_crc32_u64(crc, u64::from_le_bytes(chunk.try_into().unwrap()));
    }

    for &byte in chunks.remainder() {
        crc = _mm_crc32_u8(crc as u32, byte) as u64;
    }

    Ok((crc as u32) ^ 0xFFFFFFFF)
}

        
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all="PascalCase")]
pub struct BuildInfo {
    build: u32,
    file_hash: u32,
    name: String,
    platform: String,
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

fn get_build_path(crc: u32) -> Option<PathBuf> {
    expand_env_path(&format!(r"%LOCALAPPDATA%\Chivalry 2\Saved\Config\{:08x}.build.json", crc))
}

impl BuildInfo {
    pub fn scan() -> Self {
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

        BuildInfo {
            build: 0,
            file_hash: crc32,
            name: "".to_string(),
            platform: PLATFORM.get().unwrap_or(&PlatformType::OTHER).to_string(),
            path: file_path.to_string(),
            offsets,
        }
    }

    pub fn load(crc: u32) -> Result<Self> {
        let path = get_build_path(crc).context("Failed to expand path")?;
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let build_info: BuildInfo = serde_json::from_reader(reader)?;
        Ok(build_info)
    }

    pub fn save(&self) -> Result<()> {
        let path = get_build_path(self.file_hash).ok_or_else(|| anyhow::anyhow!("Failed to expand path"))?;
        println!("Saving build info to {}", path.to_string_lossy());
        
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
}

#[no_mangle]
pub extern "C" fn load_current_build_info() -> *const BuildInfo {
    let mut current = CURRENT_BUILD_INFO.lock().unwrap();
    if current.is_none() {
        let mut file_path = String::new();
        if let Ok(path) = env::current_exe() {
            file_path = path.to_string_lossy().into();
        }

        let crc31 = unsafe { crc32_from_file(&file_path) }.expect("Failed to compute CRC");

        match BuildInfo::load(crc31) {
            Ok(bi) => {
                *current = Some(bi);
            }
            Err(err) => {
                eprintln!("Failed to load build info: {}", err);
                eprintln!("Scanning build...");
                let bi = BuildInfo::scan();
                *current = Some(bi);
            }
        }
    }

    if let Some(ref bi) = *current {
        bi as *const BuildInfo
    } else {
        std::ptr::null()
    }
}

#[no_mangle]
pub extern "C" fn build_info_save(bi: *const BuildInfo) -> u8 {
    let bi = unsafe { &*bi };
    if let Err(e) = bi.save() {
        eprintln!("Failed to save build info: {}", e);
        return 0;
    }
    1
}

#[no_mangle]
pub extern "C" fn build_info_get_file_hash(bi: *const BuildInfo) -> u32 {
    let bi = unsafe { &*bi };
    bi.get_file_hash()
}

#[no_mangle]
pub extern "C" fn build_info_get_offset(bi: *const BuildInfo, name: *const c_char) -> u64 {
    let bi = unsafe { &*bi };
    let name = unsafe { std::ffi::CStr::from_ptr(name) }.to_string_lossy();
    *bi.get_offset(name.as_ref()).unwrap_or(&0)
}
