use crate::Chivalry2::Engine::UGameEngine;
use std::ffi::c_void;

#[unchained_macros_core::bind_func_detour(UGameEngine::DedicatedServerInit)]
pub(crate) unsafe fn dedicated_server_init_detour(_context: *const c_void) {}
