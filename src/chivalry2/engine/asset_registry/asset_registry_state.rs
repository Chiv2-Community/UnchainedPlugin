//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/AssetRegistry/Public/AssetRegistryState.h>.

/// Opaque, engine-owned Asset Registry state. It contains UE containers and
/// must be constructed/destructed by verified UE calls, never Rust allocation.
#[repr(C)]
pub struct FAssetRegistryState {
    _opaque: [u8; 0],
}

/// Opaque serialization options owned by `UAssetRegistryImpl` on the target
/// build. No Rust layout is assumed.
#[repr(C)]
pub struct FAssetRegistrySerializationOptions {
    _opaque: [u8; 0],
}
