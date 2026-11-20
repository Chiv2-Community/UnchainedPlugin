//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/GameFramework/Pawn.h>.

use crate::chivalry2::Engine::*;

use std::os::raw::c_void;

unsafe impl crate::chivalry2::Engine::UObjectType for APawn {
    const CLASS_NAME: &'static str = "Pawn";
}

/// Character

#[rust_ctables::cpp_struct]
pub struct APawn {
    #[inherit]
    pub base_actor: AActor,
    pub nav_agent: INavAgentInterface,

    // 0x0260: Bitfields (3 bits used)
    // C++: uint8 bUseControllerRotationPitch : 1;
    // C++: uint8 bUseControllerRotationYaw   : 1;
    // C++: uint8 bUseControllerRotationRoll  : 1;
    pub controller_rotation_bits: u8,

    pub padding_0: [u8; 0x3], // 0x0261 (size: 0x3)

    pub b_enabled_use_controller_rotation_pitch: bool, // 0x0264
    pub b_enabled_use_controller_rotation_yaw: bool,   // 0x0265
    pub b_enabled_use_controller_rotation_roll: bool,  // 0x0266

    pub padding_1: [u8; 0xF1], // 0x0267 (size: 0xF1)

    pub b_can_affect_navigation_generation: u8, // 0x0358
    pub padding_2: [u8; 0x3],                   // 0x0359

    pub base_eye_height: f32, // 0x035C

    pub auto_possess_player: u8, // 0x0360 (TEnumAsByte)
    pub auto_possess_ai: u8,     // 0x0361 (EAutoPossessAI)
    pub remote_view_pitch: u8,   // 0x0362

    pub padding_3: [u8; 0x5], // 0x0363

    pub ai_controller_class: *mut c_void, // 0x0368 (TSubclassOf<AController>)
    pub player_state: *mut APlayerState,  // 0x0370 (APlayerState*)

    pub padding_4: [u8; 0x8], // 0x0378

    pub last_hit_by: *mut AController, // 0x0380
    pub controller: *mut AController,  // 0x0388

    pub padding_5: [u8; 0x4], // 0x0390

    pub control_input_vector: FVector,      // 0x0394 (size: 0xC)
    pub last_control_input_vector: FVector, // 0x03A0 (size: 0xC)
} // Size: 0x3B0

impl APawn {
    /// # Safety
    /// UObjectType::from_ptr's game-thread and lifetime requirements apply.
    pub unsafe fn player_state(&self) -> Option<&APlayerState> {
        APlayerState::from_field(self.player_state, "Pawn.PlayerState")
    }

    /// # Safety
    /// UObjectType::from_ptr's game-thread and lifetime requirements apply.
    pub unsafe fn controller(&self) -> Option<&AController> {
        AController::from_ptr(self.controller)
    }

    /// Helper to check bUseControllerRotationPitch (Bit 0)
    pub fn use_controller_rotation_pitch(&self) -> bool {
        (self.controller_rotation_bits & (1 << 0)) != 0
    }

    /// Helper to check bUseControllerRotationYaw (Bit 1)
    pub fn use_controller_rotation_yaw(&self) -> bool {
        (self.controller_rotation_bits & (1 << 1)) != 0
    }

    /// Helper to check bUseControllerRotationRoll (Bit 2)
    pub fn use_controller_rotation_roll(&self) -> bool {
        (self.controller_rotation_bits & (1 << 2)) != 0
    }
}
