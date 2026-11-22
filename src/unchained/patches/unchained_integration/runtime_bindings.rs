//! Resolve runtime bindings and attach their hooks during game-thread initialization.

use std::sync::{Mutex, OnceLock};
use unchained_macros_core::PatchState;
use unchained_macros_kismet::{ExecEnvironment, KismetContext, KismetReflectionResolver};
#[cfg(feature = "blueprint_printstring_log")]
use unchained_macros_kismet::KismetExecResolver;
use crate::Chivalry2::Engine::FFrame;
use crate::unchained::blueprints::reflection::EngineReflection;
use crate::Chivalry2::Engine::Runtime::Launch::LaunchEngineLoop::FEngineLoop;

#[unchained_macros_core::bind_func_detour(FEngineLoop::Init)]
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
    let result = (|| -> Result<(), String> {
        let environment = ExecEnvironment {
            module_base: *crate::unchained::startup::BASE_ADDR,
            frame_step: FFrame::__UNCHAINED_BINDING_Step.binding().entry_address()
                .ok_or("FFrame::Step is unbound")?.get(),
            frame_step_explicit: FFrame::__UNCHAINED_BINDING_StepExplicitProperty.binding().entry_address()
                .ok_or("FFrame::StepExplicitProperty is unbound")?.get(),
            frame_code_offset: std::mem::offset_of!(FFrame, code),
        };
        let context = unsafe { KismetContext::new(EngineReflection, environment) };
        unsafe {
            unchained_macros_core::initialize_bindings_for::<KismetReflectionResolver>(&context, snapshot)?;
            crate::apply_patches::<KismetReflectionResolver>(&context, snapshot, &mut state.patches)?;
        }
        crate::attach_hooks_for::<KismetReflectionResolver>(snapshot).map_err(|error| error.to_string())?;
        #[cfg(feature = "blueprint_printstring_log")]
        if snapshot.get::<crate::PlatformType>()? == crate::PlatformType::STEAM {
            // Step adapters were attached by the early Patternsleuth pass.
            if !unchained_macros_core::detour_is_enabled("observe_frame_step")
                || !unchained_macros_core::detour_is_enabled("observe_frame_step_explicit")
            {
                return Err("Exec listeners require both active frame-step adapters".into());
            }
            unsafe { unchained_macros_core::initialize_bindings_for::<KismetExecResolver>(&context, snapshot)?; }
            crate::attach_hooks_for::<KismetExecResolver>(snapshot).map_err(|error| error.to_string())?;
        } else {
            swarn!("Blueprint print listeners are disabled: exec wrappers are verified only for Steam");
        }
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
