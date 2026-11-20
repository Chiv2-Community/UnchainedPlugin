//! Native owner: Engine/Source/Runtime/Engine/Classes/Kismet/AssemblyBlueprint.h.
//! Native source unavailable: this Chivalry engine-fork header is absent from both checkouts.

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
