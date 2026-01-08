
#[cfg(feature="server_registration")]
use std::sync::{Arc, Mutex};
use std::{env, os::raw::c_void, panic};

#[cfg(feature="mod_management")]
use crate::features::mod_management::ModManager;
#[cfg(feature="server_registration")]
use crate::features::server_registration::Registration;
use crate::{resolvers::{BASE_ADDR, PLATFORM, PlatformType}, sdebug, serror, sinfo, tools::cli_args::{CLIArgs, load_cli}, ue, ue_old::FUObjectArray};
use itertools::Itertools;
use parking_lot::RwLock;
use patternsleuth::{MemoryAccessError, disassemble::{Control, disassemble}, image::Image, resolvers::{ResolveError, impl_resolver_singleton, try_ensure_one, unreal::util}};
use patternsleuth::{ resolvers::unreal::{KismetSystemLibrary, UObjectBaseUtilityGetPathName, blueprint_library::UFunctionBind, fname::FNameToString, game_loop::{FEngineLoopInit, UGameEngineTick}, gmalloc::GMalloc, guobject_array::{FUObjectArrayAllocateUObjectIndex, FUObjectArrayFreeUObjectIndex, GUObjectArray}, kismet::{FFrameStep, FFrameStepExplicitProperty, FFrameStepViaExec}}};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, PartialEq)]
#[cfg_attr(
    feature = "serde-resolvers",
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct GEngine(pub usize);
impl_resolver_singleton!(collect, GEngine);
impl_resolver_singleton!(PEImage, GEngine, |ctx| async {
    let strings = ctx.scan(util::utf16_pattern("rhi.DumpMemory\0")).await;
    let refs = util::scan_xrefs(ctx, &strings).await;

    fn for_each(img: &Image<'_>, addr: usize) -> std::result::Result<Option<usize>, ResolveError> {
        let Some(root) = img.get_root_function(addr)? else { return Ok(None); };
        let f = root.range().start;

        let mut gengine_ptr = None;

        // Disassemble the whole function context
        disassemble(img, f, |inst| {
            let cur = inst.ip() as usize;
            if cur > addr + 0x50 { return Ok(Control::Break); } // Don't scan too far past string

            // Look for ANY IP-relative load that looks like it's pointing to a global
            // Instead of forcing RCX, let's just find the most likely GEngine xref in this block
            if inst.is_ip_rel_memory_operand() {
                let target = inst.ip_rel_memory_address() as usize;
                
                // Optional: Add a check here to see if 'target' is in the GEngine address range
                // For now, we assume the last global load before the string is our target
                gengine_ptr = Some(target);
            }

            // If we hit our string xref, we stop and take whatever global we found last
            if cur == addr {
                return Ok(Control::Break);
            }

            Ok(Control::Continue)
        })?;

        Ok(gengine_ptr)
    }

    Ok(Self(try_ensure_one(
        refs.into_iter()
            .map(|addr| for_each(ctx.image(), addr))
            .flatten_ok(),
    )?))
});
impl_resolver_singleton!(ElfImage, GEngine, |_ctx| async {
    super::bail_out!("ElfImage unimplemented");
});

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
        // g_engine: GEngine,
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

/// Pointer wrapper for things like world, gamemode
#[repr(transparent)]
#[derive(Debug)]
pub struct SyncPtr<T>(pub *mut T);

unsafe impl<T> Send for SyncPtr<T> {}
unsafe impl<T> Sync for SyncPtr<T> {}

impl<T> Copy for SyncPtr<T> {}
impl<T> Clone for SyncPtr<T> {
    fn clone(&self) -> Self { *self }
}

impl<T> Default for SyncPtr<T> {
    fn default() -> Self { Self(std::ptr::null_mut()) }
}

macro_rules! global_ptr {
    ($name:ident, $ty:ty) => {
        pub fn $name(&self) -> Option<*mut $ty> {
            let ptr = self.$name.read().0;
            if ptr.is_null() { None } else { Some(ptr) }
        }

        paste::paste! {
            pub fn [<set_ $name>](&self, ptr: *mut $ty) {
                *self.$name.write() = SyncPtr(ptr);
            }
        }
    };
}

use std::sync::OnceLock;

#[derive(Debug)]
#[allow(dead_code)]
pub struct Globals {
    resolution: DllHookResolution,
    // Use the wrapper here to satisfy the Sync requirement
    guobject_array: parking_lot::FairMutex<SyncFUObjectArray>,
    pub main_thread_id: std::thread::ThreadId,
    platform: PlatformType,
    base_address: usize,
    is_server: bool,
    pub(crate) cli_args: CLIArgs,
    pub world: RwLock<SyncPtr<c_void>>,
    #[cfg(feature="server_registration")]
    pub registration: Mutex<Option<Arc<Registration>>>,
    #[cfg(feature="mod_management")]
    pub mod_manager: Mutex<Option<Arc<ModManager>>>,
}

static GLOBALS: OnceLock<Globals> = OnceLock::new();

pub fn globals() -> &'static Globals {
    GLOBALS.get().expect("Globals not initialized")
}

#[allow(dead_code)]
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
    global_ptr!(world, c_void);
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
    sdebug!(f; "CLI Args: {:#?}", args);
    sinfo!(f; "Running resolvers");
    // let resolution = exe.resolve(DllHookResolution::resolver()).unwrap();
    let result = panic::catch_unwind(|| {
        let resolution = exe.resolve(DllHookResolution::resolver())
            .expect("Critical: Could not resolve DLL symbols"); // This panic is caught
        
        resolution
    });

    let resolution = match result {
        Ok(res) => res,
        Err(_) => {
            serror!("Global initialization failed due to a panic in resolver.");
            return Err("Global initialization aborted. Check logs.".into());
        }
    };

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
        world: RwLock::new(SyncPtr::default()),
        #[cfg(feature="server_registration")]
        registration: std::sync::Mutex::new(None),
        #[cfg(feature="mod_management")]
        mod_manager: std::sync::Mutex::new(None),
    };

    if GLOBALS.set(globals_instance).is_err() {
        eprintln!("Error: Globals already initialized!");
    }
    // sdebug!(f; "{GLOBALS:#?}");
    Ok(())
}