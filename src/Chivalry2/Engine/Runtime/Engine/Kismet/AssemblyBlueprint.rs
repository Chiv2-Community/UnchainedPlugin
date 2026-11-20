//! Native source: Engine/Source/Runtime/Engine/Classes/Kismet/AssemblyBlueprint.h.
//! Chivalry engine-fork addition.

unsafe impl crate::Chivalry2::Engine::UObjectType for UAssemblyBlueprint {
    const CLASS_NAME: &'static str = "AssemblyBlueprint";
}

#[unchained_macros::runtime_bindings]
#[rust_ctables::cpp_struct]
pub struct UAssemblyBlueprint {
    #[inherit]
    pub base: crate::Chivalry2::Engine::UBlueprint,
    // IPropertyChangedMulticast is an empty base at +0xa0; it adds no storage.
    pub is_macro: bool,
    _padding: [u8; 7],
}
