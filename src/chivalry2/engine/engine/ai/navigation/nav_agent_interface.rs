//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/AI/Navigation/NavAgentInterface.h>.

/// Secondary navigation interface; no callable slots exposed yet.
#[repr(C)]
#[derive(Debug)]
pub struct INavAgentInterface {
    vtable: *const (),
}
