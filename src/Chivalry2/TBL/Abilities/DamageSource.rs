//! Native source: TBL/Source/TBL/Abilities/DamageSource.h (shipping PDB).

#![allow(non_snake_case)]
use crate::Chivalry2::Engine::*;

unsafe impl crate::Chivalry2::Engine::UObjectType for UDamageSource {
    const CLASS_NAME: &'static str = "DamageSource";
}

/// Native damage-source prefix through Name; later fields are omitted.
#[rust_ctables::cpp_struct]
pub struct UDamageSource {
    #[inherit]
    pub base: UAssemblyBlueprint,
    pub name: FString, // 0x00a8
}
