//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/AI/Navigation/NavAgentInterface.h>.

/// Secondary navigation interface; no callable slots exposed yet.
#[unchained_macros::runtime_bindings]
#[repr(C)]
#[derive(Debug)]
pub struct INavAgentInterface {
    vtable: *const (),
}
