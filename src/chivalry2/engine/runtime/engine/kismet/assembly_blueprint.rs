//! Native source: Engine/Source/Runtime/Engine/Classes/Kismet/AssemblyBlueprint.h.
//! Chivalry engine-fork addition; header path and layout verified against shipping PDB.

unsafe impl crate::chivalry2::engine::UObjectType for UAssemblyBlueprint {
    const CLASS_NAME: &'static str = "AssemblyBlueprint";
}

#[rust_ctables::cpp_struct]
pub struct UAssemblyBlueprint {
    #[inherit]
    pub base: crate::chivalry2::engine::UBlueprint,
    // IPropertyChangedMulticast is an empty base at +0xa0; it adds no storage.
    pub is_macro: bool,
    _padding: [u8; 7],
}
