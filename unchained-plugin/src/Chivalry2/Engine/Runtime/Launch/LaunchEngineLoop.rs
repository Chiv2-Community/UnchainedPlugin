//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Launch/Public/LaunchEngineLoop.h>.

unchained_macros_patternsleuth::define_custom_resolver!(FEngineLoopInit,
    delegate patternsleuth::resolvers::unreal::game_loop::FEngineLoopInit);

/// Opaque engine-owned launch loop.
#[unchained_macros_core::runtime_bindings]
#[repr(C)]
pub struct FEngineLoop { _opaque: [u8; 0] }

#[unchained_macros_core::runtime_bindings]
impl FEngineLoop {
    /// # Safety
    /// Engine startup only, on the game thread with the live launch-loop receiver.
    #[bind_resolved_func(FEngineLoopInit)]
    pub(crate) unsafe fn Init(self: *mut Self) -> i32 {}
}
