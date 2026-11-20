//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/HAL/MemoryMisc.h>.

/// Opaque native memory-statistics map (HAL/MemoryMisc.h). Pass only a live
/// engine-constructed instance; Rust must not allocate or initialize this view.
#[unchained_macros::runtime_bindings]
#[repr(C)]
pub struct FGenericMemoryStats {
    _opaque: [u8; 0],
}
