//! UE 4.25-plus: [AActor](https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/GameFramework/Actor.h).

#[cfg(feature = "move-autonomous-desync")]
use crate::tools::hook_globals::cli_args;

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

#[cfg(feature = "move-autonomous-desync")]
define_pattern_resolver!(
    TickActor,
    ["88 54 24 10 55 53 41 54 41 55 48 8D AC 24 E8 F7 FF FF 48 81 EC 18 09 00 00 45",]
);

#[cfg(feature = "move-autonomous-desync")]
CREATE_PATCH!(TickActor, 0x73, NOP, 6, IF { cli_args().tick_actor_patch });
