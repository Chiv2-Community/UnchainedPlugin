
#[cfg(any(feature="server_registration", feature="mod_management"))]
use std::sync::Mutex;
#[cfg(feature="mod_management")]
use std::sync::Arc;
use std::{env, panic};
use crate::game::engine::UWorld;

#[cfg(feature="mod_management")]
use crate::features::mod_management::ModManager;
#[cfg(feature="server_registration")]
use crate::features::server_browser::registration::Registration;
use crate::{resolvers::{PLATFORM, PlatformType}, tools::cli_args::CLIArgs, ue};
use ue::object_array::FUObjectArray;
use parking_lot::RwLock;
use patternsleuth::resolvers::unreal::{
    UObjectBaseUtilityGetPathName,
    fname::FNameToString,
    gmalloc::GMalloc,
    guobject_array::GUObjectArray,
    kismet::{FFrameStep, FFrameStepExplicitProperty},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

patternsleuth::_impl_try_collector! {
    #[derive(Debug, PartialEq, Clone)]
    struct DllHookResolution {
        fnametostring: FNameToString,
        fframe_step: FFrameStep,
        fframe_step_explicit_property: FFrameStepExplicitProperty,
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

/// Pointer wrapper for things like world, gamemode
#[repr(transparent)]
pub struct SyncPtr<T>(pub *mut T);

impl<T> std::fmt::Debug for SyncPtr<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SyncPtr").field(&self.0).finish()
    }
}

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

macro_rules! resolution_fn {
    ($name:ident, $res_field:ident, $ty:ty) => {
        pub fn $name(&self) -> $ty {
            let ptr = usize::try_from(self.resolution.$res_field.0)
                .expect("Resolved native address does not fit a pointer");
            if ptr == 0 {
                panic!("Critical: Global resolver '{}' failed to find its target address. Check memory patterns.", stringify!($res_field));
            }
            unsafe { std::mem::transmute(ptr) }
        }
    };
}

use std::sync::OnceLock;

#[derive(Debug)]

pub struct Globals {
    resolution: DllHookResolution,
    // Use the wrapper here to satisfy the Sync requirement
    guobject_array: parking_lot::FairMutex<SyncFUObjectArray>,
    pub main_thread_id: std::thread::ThreadId,
    platform: PlatformType,
    // base_address: usize,
    is_server: bool,
    // pub(crate) cli_args: CLIArgs,
    pub world: RwLock<SyncPtr<UWorld>>,
    #[cfg(feature="server_registration")]
    pub(crate) registration: Mutex<Option<Registration>>,
    #[cfg(feature="mod_management")]
    pub mod_manager: Mutex<Option<Arc<ModManager>>>,
}

pub static CLI_ARGS: OnceLock<CLIArgs> = OnceLock::new();
static GLOBALS: OnceLock<Globals> = OnceLock::new();

pub fn cli_args() -> &'static CLIArgs {
    CLI_ARGS.get().expect("CLI_ARGS not parsed yet!")
}

pub fn globals() -> &'static Globals {
    GLOBALS.get().expect("Globals not initialized")
}


pub fn globals_initialized() -> bool {
    GLOBALS.get().is_some()
}


impl Globals {
    pub fn gmalloc(&self) -> ue::FMallocHandle {
        ue::gmalloc()
    }

    resolution_fn!(fframe_step, fframe_step, ue::FnFFrameStep);
    resolution_fn!(fframe_step_explicit_property, fframe_step_explicit_property, ue::FnFFrameStepExplicitProperty);
    resolution_fn!(fname_to_string, fnametostring, ue::FnFNameToString);
    resolution_fn!(uobject_base_utility_get_path_name, uobject_base_utility_get_path_name, ue::FnUObjectBaseUtilityGetPathName);

    pub fn guobject_array(&self) -> parking_lot::FairMutexGuard<'_, SyncFUObjectArray> {
        self.guobject_array.lock()
    }
    /// # Safety
    /// Caller must ensure no concurrent mutable access to the object array.
    pub unsafe fn guobject_array_unchecked(&self) -> &FUObjectArray {
        // deref ptr to SyncFUObjectArray, then deref to FUObjectArray
        &( *self.guobject_array.data_ptr() )
    }
    pub fn get_platform(&self) -> PlatformType {
        self.platform
    }
    pub fn is_server(&self) -> bool {
        self.is_server
    }
    // fn args(&self) -> &CLIArgs {
    //     &self.cli_args
    // }
    global_ptr!(world, UWorld);
}

pub fn init_globals() -> Result<(), String> {
    let platform = match env::args().any(|arg| arg == "-epicapp=Peppermint") {
        true => PlatformType::EGS,
        false => PlatformType::STEAM,
    };

    let exe = patternsleuth::process::internal::read_image()
        .map_err(|e| e.to_string())
        .expect("failed to read image");
    // FIXME: replace old references
    PLATFORM.set(platform).expect("Platform already set");
    sinfo!(
        "Platform: {} base_addr: '0x{:x?}'",
        platform, *crate::resolvers::BASE_ADDR
    );

    // Load CLI ARGS
    sinfo!("Running resolvers");
    // Required values are fatal on resolution failure, including standalone
    // initialization that has not run the cached preinit binding pass.
    let gmalloc = exe.resolve(GMalloc::resolver())
        .unwrap_or_else(|error| panic!("Required native value GMalloc could not be resolved: {error}"));
    unsafe { crate::resolvers::initialize_value_binding("GMalloc",
        usize::try_from(gmalloc.0).expect("GMalloc address does not fit a pointer")); }
    let guobject_array = exe.resolve(GUObjectArray::resolver())
        .unwrap_or_else(|error| panic!("Required native value GUObjectArray could not be resolved: {error}"));
    unsafe { crate::resolvers::initialize_value_binding("GUObjectArray",
        usize::try_from(guobject_array.0).expect("GUObjectArray address does not fit a pointer")); }
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

    let guobject_array: &'static FUObjectArray =
        unsafe { &*ue::object_array::native_guobject_array() };

    let globals_instance = Globals {
        // Wrap the reference in our Sync-promising struct
        guobject_array: parking_lot::FairMutex::new(SyncFUObjectArray(guobject_array)),
        resolution,
        main_thread_id: std::thread::current().id(),
        is_server: false,
        // cli_args: args,
        platform,
        world: RwLock::new(SyncPtr::default()),
        #[cfg(feature="server_registration")]
        registration: std::sync::Mutex::new(None),
        #[cfg(feature="mod_management")]
        mod_manager: std::sync::Mutex::new(None),
    };

    if GLOBALS.set(globals_instance).is_err() {
        serror!("Error: Globals already initialized!");
    }
    strace!("{GLOBALS:#?}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn world_pointer_contracts() {
        let _: fn(&Globals) -> Option<*mut UWorld> = Globals::world;
        let _: fn(&Globals, *mut UWorld) = Globals::set_world;
        let _: fn(&Globals) -> &RwLock<SyncPtr<UWorld>> = |globals| &globals.world;
        let _: fn(*mut UWorld) -> Option<String> =
            crate::ue::engine::uworld::get_current_map_name;
    }

    #[test]
    fn pointer_debug_does_not_require_pointee_debug() {
        struct NoDebug;
        let pointer = SyncPtr::<NoDebug>::default();
        assert_eq!(format!("{pointer:?}"), format!("SyncPtr({:?})", pointer.0));
    }
}
