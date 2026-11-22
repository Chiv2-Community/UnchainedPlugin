//! Resolve runtime bindings and attach their hooks during game-thread initialization.

use std::sync::{Mutex, OnceLock};
use unchained_macros_core::PatchState;
use unchained_macros_kismet::{KismetContext, KismetReflectionResolver};
use crate::Chivalry2::Engine::Runtime::Launch::LaunchEngineLoop::FEngineLoop;

#[unchained_macros::bind_func_detour(FEngineLoop::Init)]
unsafe fn engine_loop_init_detour(engine: *mut FEngineLoop) -> i32 {
    unsafe { initialize(); }
    let result = unsafe { engine.Init() };
    // Retry only unresolved work if some classes were unavailable at entry.
    unsafe { initialize(); }
    result
}

#[derive(Default)]
struct State {
    complete: bool,
    patches: PatchState,
    last_error: Option<String>,
}

/// # Safety
/// Invoke on the game thread at startup while native callers are quiescent.
pub(super) unsafe fn initialize() {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    let Ok(mut state) = STATE.get_or_init(Default::default).try_lock() else { return };
    if state.complete { return; }
    let snapshot = crate::selectors();
    let context = unsafe { KismetContext::new(crate::unchained::reflection::resolve_native) };
    let result = (|| -> Result<(), String> {
        unsafe {
            unchained_macros_core::initialize_bindings_for::<KismetReflectionResolver>(&context, snapshot)?;
            crate::apply_patches::<KismetReflectionResolver>(&context, snapshot, &mut state.patches)?;
        }
        crate::attach_hooks_for::<KismetReflectionResolver>(snapshot).map_err(|error| error.to_string())?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            state.complete = true;
            sinfo!("Runtime native bindings initialized.");
        }
        Err(error) => {
            if state.last_error.as_ref() != Some(&error) {
                swarn!("Runtime native initialization pending: {}", error);
                state.last_error = Some(error);
            }
        }
    }
}
