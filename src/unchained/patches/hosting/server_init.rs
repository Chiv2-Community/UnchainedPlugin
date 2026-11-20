use crate::chivalry2::engine::UGameEngine;
use std::ffi::c_void;

#[unchained_macros::bind_func_detour(UGameEngine::dedicated_server_init)]
pub(crate) unsafe fn dedicated_server_init_detour(_context: *const c_void) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_static_signature() {
        let _: unsafe fn(*const c_void) = UGameEngine::dedicated_server_init;
        let _: unsafe fn(*const c_void) = dedicated_server_init_detour;
    }
}
