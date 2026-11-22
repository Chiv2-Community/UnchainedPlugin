#![doc = include_str!("../README.md")]

pub use unchained_macros_kismet_impl::kismet_runtime_bindings;
use std::{num::NonZeroUsize, sync::{Mutex, OnceLock}};

/// Game-thread lookup supplied by the consuming application's native object model.
pub struct KismetContext {
    lookup: unsafe fn(&str, &str) -> Result<NonZeroUsize, String>,
    _game_thread: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl KismetContext {
    /// # Safety
    /// Construct and use on the game thread once reflection is ready.
    /// Native callers must remain quiescent until bindings and hooks are published.
    pub unsafe fn new(lookup: unsafe fn(&str, &str) -> Result<NonZeroUsize, String>) -> Self {
        Self { lookup, _game_thread: std::marker::PhantomData }
    }
}

pub struct KismetReflectionResolver {
    pub owner: &'static str,
    pub member: &'static str,
    address: OnceLock<NonZeroUsize>,
    lock: Mutex<()>,
}
impl KismetReflectionResolver {
    pub const fn new(owner: &'static str, member: &'static str) -> Self {
        Self { owner, member, address: OnceLock::new(), lock: Mutex::new(()) }
    }
}
impl unchained_macros_core::AddressResolver for KismetReflectionResolver {
    type Context = KismetContext;
    unsafe fn resolve(&self, context: &KismetContext) -> Result<NonZeroUsize, String> {
        let _guard = self.lock.lock().map_err(|_| "Kismet resolver lock poisoned")?;
        if let Some(address) = self.address.get() { return Ok(*address); }
        let address = unsafe { (context.lookup)(self.owner, self.member)? };
        self.address.set(address).map_err(|_| "Kismet resolver was already published")?;
        Ok(address)
    }
}

#[cfg(any(windows, test))]
mod exec_thunk;
#[cfg(windows)]
pub use exec_thunk::native_body;
