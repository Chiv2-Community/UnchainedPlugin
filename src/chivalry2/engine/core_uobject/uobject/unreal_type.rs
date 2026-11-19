//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/UObject/UnrealType.h>.

use crate::chivalry2::engine::*;

/// Prefix of UE 4.25 `FProperty`; later members are not needed.
#[repr(C)]
pub struct FProperty {
    pub field: FField,
    pub array_dim: i32,
    pub element_size: i32,
    pub property_flags: u64,
    pub rep_index: u16,
    pub blueprint_replication_condition: u8,
    /// Byte offset of the value inside its owning object.
    pub offset_internal: i32,
}

const _: () = assert!(std::mem::offset_of!(FProperty, offset_internal) == 0x4C);
