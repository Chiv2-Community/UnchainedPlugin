//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/GameFramework/Actor.h>.

// Native AActor size: 0x258; APawn/AController add INavAgentInterface at +0x258.
#[unchained_macros_core::runtime_bindings]
#[rust_ctables::cpp_struct]
#[derive(Debug)]
pub struct AActor {
    #[inherit]
    pub base: crate::Chivalry2::Engine::UObject,
    _private: [u8; 0x230],
}

// SAFETY: This repr(C) layout models the native class prefix at offset zero.
unsafe impl crate::Chivalry2::Engine::UObjectType for AActor {
    const CLASS_NAME: &'static str = "Actor";
}
