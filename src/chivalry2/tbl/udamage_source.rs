//! Native source: <https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/DamageSource.h>.

#![allow(non_snake_case)]
use crate::chivalry2::engine::*;

unsafe impl crate::chivalry2::engine::UObjectType for UDamageSource {
    const CLASS_NAME: &'static str = "DamageSource";
}

/// Native damage-source prefix through Name; later fields are omitted.
#[rust_ctables::cpp_struct]
pub struct UDamageSource {
    #[inherit]
    pub base: UAssemblyBlueprint,
    pub name: FString, // 0x00a8
}
