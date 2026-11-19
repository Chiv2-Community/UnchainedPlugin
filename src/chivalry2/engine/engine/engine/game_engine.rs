//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/Engine/GameEngine.h>.

/// Opaque engine-owned game engine; no Rust layout is assumed.
#[repr(C)]
pub struct UGameEngine {
    _opaque: [u8; 0],
}

#[unchained_macros::runtime_bindings]
impl UGameEngine {
    #[unchained_macros::bind_resolved_func(
        patternsleuth::resolvers::unreal::game_loop::UGameEngineTick
    )]
    pub(crate) unsafe fn tick(self: *mut Self, delta: f32, state: u8) {}
}
