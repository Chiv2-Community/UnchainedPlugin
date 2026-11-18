//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/GameFramework/Controller.h>.

#![allow(non_snake_case)]
use crate::engine::*;

// SAFETY: These repr(C) layouts model the supported native class prefixes at offset zero.
unsafe impl crate::ue::UObjectType for AController {
    const CLASS_NAME: &'static str = "Controller";
}

#[rust_ctables::cpp_struct]
#[derive(Debug)]
pub struct AController {
    #[inherit]
    pub base: AActor,
    pub nav_agent: INavAgentInterface,
    // 0x0260: APlayerState* PlayerState (UE4 Controller.h)
    pub player_state: *mut APlayerState,
    _padding_to_pawn: [u8; 0x20],
    pub pawn: *mut APawn, // 0x0288
    _old_pawn: [u8; 8],
    pub character: *mut ACharacter, // 0x0298
    _remaining: [u8; 0x30],
}

impl AController {
    /// # Safety
    /// UObjectType::from_ptr's game-thread and lifetime requirements apply.
    pub unsafe fn player_state(&self) -> Option<&APlayerState> {
        APlayerState::from_field(self.player_state, "Controller.PlayerState")
    }

    /// # Safety
    /// UObjectType::from_ptr's game-thread and lifetime requirements apply.
    pub unsafe fn pawn(&self) -> Option<&APawn> {
        APawn::from_ptr(self.pawn)
    }
}
