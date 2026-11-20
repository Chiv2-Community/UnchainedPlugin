//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/GameFramework/Character.h>.

use crate::Chivalry2::Engine::*;

use bitflags::bitflags;

use std::os::raw::c_void;

unsafe impl crate::Chivalry2::Engine::UObjectType for ACharacter {
    const CLASS_NAME: &'static str = "Character";
}

bitflags! {
    /// Character movement bitfield 1 (offset 0x0460)
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CharacterMovementFlags1: u8 {
        const IS_CROUCHED = 1 << 0;
        const PROXY_IS_JUMP_FORCE_APPLIED = 1 << 1;
        const PRESSED_JUMP = 1 << 2;
        const CLIENT_UPDATING = 1 << 3;
        const CLIENT_WAS_FALLING = 1 << 4;
        const CLIENT_RESIMULATE_ROOT_MOTION = 1 << 5;
        const CLIENT_RESIMULATE_ROOT_MOTION_SOURCES = 1 << 6;
        const SIM_GRAVITY_DISABLED = 1 << 7;
    }
}

bitflags! {
    /// Character movement bitfield 2 (offset 0x0461)
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CharacterMovementFlags2: u8 {
        const CLIENT_CHECK_ENCROACHMENT_ON_NET_UPDATE = 1 << 0;
        const SERVER_MOVE_IGNORE_ROOT_MOTION = 1 << 1;
        const WAS_JUMPING = 1 << 2;
    }
}

#[rust_ctables::cpp_struct]
pub struct ACharacter {
    #[inherit]
    pub base_pawn: APawn,

    pub mesh: *mut c_void,               // 0x03B0 (USkeletalMeshComponent*)
    pub character_movement: *mut c_void, // 0x03B8 (UCharacterMovementComponent*)
    pub capsule_component: *mut c_void,  // 0x03C0 (UCapsuleComponent*)

    pub based_movement: [u8; 0x30], // 0x03C8 (FBasedMovementInfo)
    pub replicated_based_movement: [u8; 0x30], // 0x03F8 (FBasedMovementInfo)

    pub padding_0: [u8; 0x4],                    // 0x0428
    pub anim_root_motion_translation_scale: f32, // 0x042C
    pub base_translation_offset: FVector,        // 0x0430 (size: 0xC)
    pub padding_1: [u8; 0x4],                    // 0x043C
    pub base_rotation_offset: [u8; 0x10],        //FQuat,     // 0x0440 (size: 0x10)

    pub replicated_server_last_transform_update_timestamp: f32, // 0x0450
    pub replay_last_transform_update_timestamp: f32,            // 0x0454

    pub replicated_movement_mode: u8, // 0x0458
    pub b_in_base_replication: bool,  // 0x0459
    pub padding_2: [u8; 0x2],         // 0x045A
    pub crouched_eye_height: f32,     // 0x045C

    pub movement_flags_1: CharacterMovementFlags1, // 0x0460
    pub movement_flags_2: CharacterMovementFlags2, // 0x0461

    pub padding_3: [u8; 0x2],               // 0x0462
    pub jump_key_hold_time: f32,            // 0x0464
    pub jump_force_time_remaining: f32,     // 0x0468
    pub proxy_jump_force_started_time: f32, // 0x046C
    pub jump_max_hold_time: f32,            // 0x0470
    pub jump_max_count: i32,                // 0x0474
    pub jump_current_count: i32,            // 0x0478

    pub padding_4: [u8; 0x4],             // 0x047C
    pub on_reached_jump_apex: [u8; 0x10], // 0x0480 (Delegate)
    pub padding_5: [u8; 0x10],            // 0x0490

    pub movement_mode_changed_delegate: [u8; 0x10], // 0x04A0
    pub on_character_movement_updated: [u8; 0x10],  // 0x04B0

    pub saved_root_motion: [u8; 0x38], // 0x04C0 (FRootMotionSourceGroup)
    pub padding_6: [u8; 0x8],          // 0x04F8
    pub client_root_motion_params: [u8; 0x40], // 0x0500 (FRootMotionMovementParams)
    pub root_motion_rep_moves: [u8; 0x10], // 0x0540 (TArray)
    pub rep_root_motion: [u8; 0x98],   // 0x0550 (FRepRootMotionMontage)

    pub final_padding: [u8; 0x8], // 0x05E8 (Aligns to size 0x5F0)
} // Size: 0x5F0

impl ACharacter {
    // Offset 0x0460 Helpers
    pub fn is_crouched(&self) -> bool {
        self.movement_flags_1
            .contains(CharacterMovementFlags1::IS_CROUCHED)
    }
    pub fn proxy_is_jump_force_applied(&self) -> bool {
        self.movement_flags_1
            .contains(CharacterMovementFlags1::PROXY_IS_JUMP_FORCE_APPLIED)
    }
    pub fn pressed_jump(&self) -> bool {
        self.movement_flags_1
            .contains(CharacterMovementFlags1::PRESSED_JUMP)
    }
    pub fn client_updating(&self) -> bool {
        self.movement_flags_1
            .contains(CharacterMovementFlags1::CLIENT_UPDATING)
    }
    pub fn client_was_falling(&self) -> bool {
        self.movement_flags_1
            .contains(CharacterMovementFlags1::CLIENT_WAS_FALLING)
    }
    pub fn sim_gravity_disabled(&self) -> bool {
        self.movement_flags_1
            .contains(CharacterMovementFlags1::SIM_GRAVITY_DISABLED)
    }

    // Offset 0x0461 Helpers
    pub fn client_check_encroachment(&self) -> bool {
        self.movement_flags_2
            .contains(CharacterMovementFlags2::CLIENT_CHECK_ENCROACHMENT_ON_NET_UPDATE)
    }
    pub fn server_move_ignore_root_motion(&self) -> bool {
        self.movement_flags_2
            .contains(CharacterMovementFlags2::SERVER_MOVE_IGNORE_ROOT_MOTION)
    }
    pub fn was_jumping(&self) -> bool {
        self.movement_flags_2
            .contains(CharacterMovementFlags2::WAS_JUMPING)
    }
}
