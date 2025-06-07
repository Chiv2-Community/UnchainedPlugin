

mod resolvers;
mod scan;
use std::env;
use std::fs::File;
use std::io::{BufReader, Read};
use std::io::{BufWriter, Write};
use std::collections::HashMap;
use std::path::PathBuf;
mod ue;

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
use serde::Serialize;
use serde_json::to_writer_pretty;
use winapi::um::winnt::FILE_APPEND_DATA;
use self::resolvers::{PLATFORM, BASE_ADDR, PlatformType};

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
    println!("JSON PATH {}", builds_path.to_string_lossy());
    let file = File::create(builds_path)?;
    let mut writer = BufWriter::new(file);
    let mut data = HashMap::new();

    // let mut file_path = String::new();

    let offsets = crate::scan::OFFSETS.get().unwrap();
    let base_addr = BASE_ADDR.get().unwrap();

    let mut file_path: String = env::current_exe().unwrap().to_string_lossy().into();

    // match env::current_exe() {
    //     Ok(path) => file_path = path.to_string_lossy().into(),
    //     Err(e) => eprintln!("Failed to get path: {}", e),
    // }
    println!("Current executable path: {:?}", file_path);
    
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
    resolvers::admin_control::attach_UGameEngineTick(base_address, offsets.clone()).unwrap();
    resolvers::admin_control::attach_ExecuteConsoleCommand(base_address, offsets.clone()).unwrap();
    resolvers::admin_control::attach_FEngineLoopInit(base_address, offsets).unwrap();
    Ok(())
  }

unsafe fn init_globals() {
    
    let exe = patternsleuth::process::internal::read_image().map_err(|e| e.to_string()).expect("failed to read image");
    println!("starting scan");
    let resolution = exe.resolve(DllHookResolution::resolver()).unwrap();
    println!("finished scan");

    println!("results: {:?}", resolution);
    let guobject_array: &'static ue::FUObjectArray =
        &*(resolution.guobject_array.0 as *const ue::FUObjectArray);

    GLOBALS = Some(Globals {
        guobject_array: guobject_array.into(),
        resolution,
        main_thread_id: std::thread::current().id(),
        last_command: None,
    });

}

#[no_mangle]
pub extern "C" fn generate_json() -> u8 {
    println!("test asd");
    
    let platform = match env::args().any(|arg| arg == "-epicapp=Peppermint") {
        true => PlatformType::EGS,
        false => PlatformType::STEAM
    };

    std::thread::spawn(|| {
        resolvers::rcon::handle_rcon();
    });

    PLATFORM.set(platform).expect("Platform already set");
    let image = patternsleuth::process::internal::read_image().map_err(|e| e.to_string()).expect("failed to read image");
    let exe = image;
    println!("GAME  '{:x?}'", exe.base_address);
    BASE_ADDR.set(exe.base_address).expect("BASE_ADDR already set");

    unsafe { init_globals() };
    // let scan = scan::scan();
    let offsets = scan::scan().expect("Failed to scan");
    let len_u8 = offsets.len() as u8;
    // FIXME: ?
    let offset_copy = offsets.clone();
    // let base_addr = scan::scan().1;
    unsafe {
        attach_hooks(exe.base_address, offsets).unwrap();
    }
    dump_builds().expect("Failed to dump builds JSON");
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
    main_thread_id: std::thread::ThreadId,
    last_command: Option<ue::FString>,
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

    // pub fn last_command(&self) -> Option<&mut ue::FString> {
    //     self.last_command.as_mut()
    // }
}

pub fn globals() -> &'static Globals {
    unsafe { GLOBALS.as_ref().unwrap() }
}