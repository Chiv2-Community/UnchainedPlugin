//! Native source: <https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/TBLCharacterBase.h>.
//! Native owner: TBL/Source/TBL/TBLCharacterBase.h.

#![allow(non_snake_case)]
use crate::Chivalry2::Engine::*;
use crate::Chivalry2::TBL::*;
use std::os::raw::c_void;

unsafe impl crate::Chivalry2::Engine::UObjectType for ATBLCharacterBase {
    const CLASS_NAME: &'static str = "TBLCharacterBase";
}

#[unchained_macros_core::runtime_bindings]
#[rust_ctables::cpp_struct]
pub struct ATBLCharacterBase {
    #[inherit]
    pub base_character: ACharacter,

    pub significance_state: [u8; 0x28], // 0x05F0 (FCharacterSignificanceState)
    pub movement_base: *mut c_void,     // 0x0618 (UTBLCharacterMovementBaseComponent*)
    pub view_distance: f32,             // 0x0620
    pub padding_0: [u8; 0x4],           // 0x0624

    pub on_recently_rendered_delegate: [u8; 0x10], // 0x0628 (Delegate)
    pub b_was_recently_rendered: bool,             // 0x0638
    pub padding_1: [u8; 0x3],                      // 0x0639

    pub last_anim_update_rate: i32, // 0x063C
    pub padding_2: [u8; 0x8],       // 0x0640

    // State Flags (Individual booleans, not bitfields in this specific class)
    pub b_root_motion_active: bool,      // 0x0648
    pub b_montage_active: bool,          // 0x0649
    pub b_weapon_tracers_active: bool,   // 0x064A
    pub b_waiting_for_anim_notify: bool, // 0x064B
    pub b_ignore_face_rotation: bool,    // 0x064C
    pub b_is_idle_animation: bool,       // 0x064D
    pub b_is_carryable_npc: bool,        // 0x064E

    pub padding_3: [u8; 0x1],     // 0x064F
    pub last_carry_time: f32,     // 0x0650
    pub mesh_visibility_flag: u8, // 0x0654 (EMeshVisibilityFlag)
    pub padding_4: [u8; 0x3],     // 0x0655

    pub hidden_anim_update_max_distance: f32, // 0x0658
    pub b_kill_upon_landing: bool,            // 0x065C
    pub padding_5: [u8; 0x3],                 // 0x065D

    // Fall Damage Logic
    pub fall_damage_starting_speed: f32,                 // 0x0660
    pub padding_6: [u8; 0x4],                            // 0x0664
    pub fall_damage_source: *mut UDamageSource,          // 0x0668
    pub falling_damage_phys_mat_multipliers: [u8; 0x10], // 0x0670 (TArray)

    pub skip_client_optimizations: bool, // 0x0680
    pub b_placed_in_world: bool,         // 0x0681
    pub padding_7: [u8; 0x6],            // 0x0682

    pub on_root_transform_updated: [u8; 0x10], // 0x0688 (Delegate)
    pub padding_8: [u8; 0x4],                  // 0x0698

    pub fall_damage_scale: f32,         // 0x069C
    pub default_fall_damage_scale: f32, // 0x06A0

    pub final_padding: [u8; 0xC], // 0x06A4 to 0x06B0
} // Size: 0x6B0
