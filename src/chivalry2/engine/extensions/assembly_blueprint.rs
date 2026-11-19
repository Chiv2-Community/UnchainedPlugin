//! ArgonSDK consumer: [UDamageSource](https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/DamageSource.h).
//! UAssemblyBlueprint's declaration is not shipped; this prefix is reverse-engineered.

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
