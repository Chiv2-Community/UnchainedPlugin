//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/AssetRegistry/Public/AssetRegistryState.h>.

/// Opaque, engine-owned Asset Registry state. It contains UE containers and
/// must be constructed/destructed by verified UE calls, never Rust allocation.
#[unchained_macros_core::runtime_bindings]
#[repr(C)]
pub struct FAssetRegistryState {
    _opaque: [u8; 0],
}

/// Opaque serialization options owned by `UAssetRegistryImpl` on the target
/// build. No Rust layout is assumed.
#[unchained_macros_core::runtime_bindings]
#[repr(C)]
pub struct FAssetRegistrySerializationOptions {
    _opaque: [u8; 0],
}
