
use std::env;

use crate::{resolvers::{BASE_ADDR, PLATFORM, PlatformType}, sinfo, swarn, tools::cli_args::{CLIArgs, load_cli}, ue, ue_old::FUObjectArray};
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

static mut GLOBALS: Option<Globals> = None;

#[derive(Debug)]
#[allow(dead_code)]
pub struct Globals {
    resolution: DllHookResolution,
    guobject_array: parking_lot::FairMutex<&'static FUObjectArray>,
    #[allow(dead_code)]
    main_thread_id: std::thread::ThreadId,
    // last_command: Option<ue::FString>,
    platform: PlatformType,
    base_address: usize,
    is_server: bool,
    pub(crate) cli_args: CLIArgs,
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
    pub fn guobject_array(&self) -> parking_lot::FairMutexGuard<'static, &FUObjectArray> {
        self.guobject_array.lock()
    }
    pub unsafe fn guobject_array_unchecked(&self) -> &FUObjectArray {
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
    unsafe { GLOBALS.as_ref().expect("Globals not initialized") }
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
    sinfo!(
        "Platform: {} base_addr: '0x{:x?}'",
        platform, exe.base_address
    );

    // Load CLI ARGS
    let args = load_cli().expect("Failed to load CLI ARGS");
    sdebug!(f; "CLI Args: {:?}", args);
    sinfo!(f; "Running resolvers");
    let resolution = exe.resolve(DllHookResolution::resolver()).unwrap();
    // use rayon::ThreadPoolBuilder;

    // let pool = ThreadPoolBuilder::new().num_threads(1).build().unwrap();
    // pool.install(|| {
    //     let resolution = exe.resolve(DllHookResolution::resolver()).unwrap();
    // });
    // let resolution = match exe.resolve(DllHookResolution::resolver()) {
    //     Ok(res) => {
    //         println!("Resolvers complete");
    //         res
    //     }
    //     Err(e) => {
    //         eprintln!("Failed to resolve: {}", e);
    //         return Err(clap::error::Error::raw(
    //             clap::error::ErrorKind::Io,
    //             format!("Failed to resolve: {}", e),
    //         ));
    //     }
    // };
    // let resolution = exe.resolve(DllHookResolution::resolver()).unwrap();
    log::info!("Resolvers complete ASDF");
    sinfo!("Resolvers complete");
    // println!("results: {:?}", resolution);
    let guobject_array: &'static FUObjectArray =
        &*(resolution.guobject_array.0 as *const FUObjectArray);

    GLOBALS = Some(Globals {
        guobject_array: guobject_array.into(),
        resolution,
        main_thread_id: std::thread::current().id(),
        // last_command: None,
        base_address: exe.base_address,
        is_server: false,
        cli_args: args,
        platform,
    });
    // sdebug!(f; "{GLOBALS:#?}");
    Ok(())
}