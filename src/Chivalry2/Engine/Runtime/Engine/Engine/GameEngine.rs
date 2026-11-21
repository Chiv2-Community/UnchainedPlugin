//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/Engine/GameEngine.h>.

use std::ffi::c_void;

unchained_macros::define_custom_resolver!(UGameEngineTick,
    delegate patternsleuth::resolvers::unreal::game_loop::UGameEngineTick);

define_pattern_resolver!(AllowAlternateBackend, [
    "48 8B C4 55 48 8D 68 ?? 48 81 EC C0 00 00 00 48 89 58 ?? 48 89 70 ?? 48 89 78 ?? 4C 89 60 ?? 4C 89 70"
]);

define_pattern_resolver!(DedicatedServerInit, [
    "41 56 48 83 EC 30 48 89 6C 24 48 48 89 74 24 50 48 89 7C 24 28 4C 89 7C 24 20 4C 8B F9 ?? ?? ?? ?? ?? 48 8B C8 4C 8B F0"
]);

/// Opaque engine-owned game engine; no Rust layout is assumed.
#[unchained_macros::runtime_bindings]
#[repr(C)]
pub struct UGameEngine {
    _opaque: [u8; 0],
}

#[unchained_macros::runtime_bindings]
impl UGameEngine {
    #[unchained_macros::bind_resolved_func(DedicatedServerInit)]
    pub(crate) unsafe fn DedicatedServerInit(context: *const c_void) {}

    #[unchained_macros::bind_resolved_func(
        self::UGameEngineTick
    )]
    pub(crate) unsafe fn Tick(self: *mut Self, delta: f32, state: u8) {}
}
