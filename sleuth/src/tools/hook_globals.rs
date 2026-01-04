
use std::env;

use crate::{resolvers::{BASE_ADDR, PLATFORM, PlatformType}, sdebug, sinfo, tools::cli_args::{CLIArgs, load_cli}, ue, ue_old::FUObjectArray};
use patternsleuth::resolvers::unreal::{KismetSystemLibrary, UObjectBaseUtilityGetPathName, blueprint_library::UFunctionBind, fname::FNameToString, game_loop::{FEngineLoopInit, UGameEngineTick}, gmalloc::GMalloc, guobject_array::{FUObjectArrayAllocateUObjectIndex, FUObjectArrayFreeUObjectIndex, GUObjectArray}, kismet::{FFrameStep, FFrameStepExplicitProperty, FFrameStepViaExec}};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

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

// Stubs to disable unimplemented err
impl Serialize for DllHookResolution {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        todo!("Serialization not implemented")
    }
}
impl<'de> Deserialize<'de> for DllHookResolution {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        todo!("Deserialization not implemented")
    }
}


#[repr(transparent)]
#[derive(Debug)]
pub struct SyncFUObjectArray(&'static FUObjectArray);
unsafe impl Sync for SyncFUObjectArray {}
unsafe impl Send for SyncFUObjectArray {}

impl std::ops::Deref for SyncFUObjectArray {
    type Target = FUObjectArray;
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

use std::sync::OnceLock;

#[derive(Debug)]
#[allow(dead_code)]
pub struct Globals {
    resolution: DllHookResolution,
    // Use the wrapper here to satisfy the Sync requirement
    guobject_array: parking_lot::FairMutex<SyncFUObjectArray>,
    main_thread_id: std::thread::ThreadId,
    platform: PlatformType,
    base_address: usize,
    is_server: bool,
    pub(crate) cli_args: CLIArgs,
}

static GLOBALS: OnceLock<Globals> = OnceLock::new();

pub fn globals() -> &'static Globals {
    GLOBALS.get().expect("Globals not initialized")
}

pub fn globals_initialized() -> bool {
    GLOBALS.get().is_some()
}

#[allow(dead_code)]
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
    pub fn guobject_array(&self) -> parking_lot::FairMutexGuard<'_, SyncFUObjectArray> {
        self.guobject_array.lock()
    }
    pub unsafe fn guobject_array_unchecked(&self) -> &FUObjectArray {
        // deref ptr to SyncFUObjectArray, then deref to FUObjectArray
        &( *self.guobject_array.data_ptr() )
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

pub unsafe fn init_globals() -> Result<(), clap::error::Error> {
    let platform = match env::args().any(|arg| arg == "-epicapp=Peppermint") {
        true => PlatformType::EGS,
        false => PlatformType::STEAM,
    };

    let exe = patternsleuth::process::internal::read_image()
        .map_err(|e| e.to_string())
        .expect("failed to read image");
    // FIXME: replace old references
    PLATFORM.set(platform).expect("Platform already set");
    BASE_ADDR.set(exe.base_address).expect("BASE_ADDR already set");
    sinfo!(f;
        "Platform: {} base_addr: '0x{:x?}'",
        platform, exe.base_address
    );

    // Load CLI ARGS
    let args = load_cli().expect("Failed to load CLI ARGS");
    sdebug!(f; "CLI Args: {:?}", args);
    sinfo!(f; "Running resolvers");
    let resolution = exe.resolve(DllHookResolution::resolver()).unwrap();
    // println!("results: {:?}", resolution);
    let guobject_array: &'static FUObjectArray =
        &*(resolution.guobject_array.0 as *const FUObjectArray);

    let globals_instance = Globals {
        // Wrap the reference in our Sync-promising struct
        guobject_array: parking_lot::FairMutex::new(SyncFUObjectArray(guobject_array)),
        resolution,
        main_thread_id: std::thread::current().id(),
        base_address: exe.base_address,
        is_server: false,
        cli_args: args,
        platform,
    };

    if GLOBALS.set(globals_instance).is_err() {
        eprintln!("Error: Globals already initialized!");
    }
    // sdebug!(f; "{GLOBALS:#?}");
    Ok(())
}