
#[cfg(any(feature="server_registration", feature="mod_management"))]
use std::sync::Mutex;
#[cfg(feature="mod_management")]
use std::sync::Arc;
use crate::Chivalry2::Engine::UWorld;

#[cfg(feature="mod_management")]
use crate::features::mod_management::ModManager;
#[cfg(feature="server_registration")]
use crate::features::server_browser::registration::Registration;
use crate::tools::cli_args::CLIArgs;
use parking_lot::RwLock;


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

use std::sync::OnceLock;

#[derive(Debug)]

pub struct Globals {
    pub main_thread_id: std::thread::ThreadId,
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
    pub fn world(&self) -> Option<*mut UWorld> {
        let ptr = self.world.read().0;
        if ptr.is_null() { None } else { Some(ptr) }
    }

    pub fn set_world(&self, ptr: *mut UWorld) {
        *self.world.write() = SyncPtr(ptr);
    }
}

pub fn init_globals() {
    sinfo!(
        "Platform: {} base_addr: '0x{:x?}'",
        crate::current_platform(), *crate::unchained::startup::BASE_ADDR
    );

    let globals_instance = Globals {
        main_thread_id: std::thread::current().id(),
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_debug_does_not_require_pointee_debug() {
        struct NoDebug;
        let pointer = SyncPtr::<NoDebug>::default();
        assert_eq!(format!("{pointer:?}"), format!("SyncPtr({:?})", pointer.0));
    }
}
