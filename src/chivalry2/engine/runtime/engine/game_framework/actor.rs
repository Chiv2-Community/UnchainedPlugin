//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/GameFramework/Actor.h>.

// Native PDB: AActor 0x258; APawn/AController add INavAgentInterface at +0x258.
#[rust_ctables::cpp_struct]
#[derive(Debug)]
pub struct AActor {
    #[inherit]
    pub base: crate::chivalry2::engine::UObject,
    _private: [u8; 0x230],
}

// SAFETY: This repr(C) layout models the native class prefix at offset zero.
unsafe impl crate::chivalry2::engine::UObjectType for AActor {
    const CLASS_NAME: &'static str = "Actor";
}
