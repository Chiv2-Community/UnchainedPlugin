//! Native source: <https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/DamageTakenEvent.h>.

#![allow(non_snake_case)]
use crate::chivalry2::engine::*;
use crate::chivalry2::tbl::*;
use std::os::raw::c_void;

/// Layout based on `CXXHeaderDump/TBL.hpp` (`Size: 0x148`).
#[repr(C)]
#[derive(Debug)]
pub struct FDamageTakenEvent {
    pub damage: f32,
    pub new_stat_value: f32,
    pub damage_source: *mut UDamageSource,
    pub damage_causer: *mut AActor,     // AActor*
    pub damage_taker: *mut AActor,      // AActor*
    pub damage_instigator: *mut AActor, // AActor*
    pub b_killing_blow: bool,
    pub b_suicide: bool,
    pub b_back_stab: bool,
    pub b_entered_kill_volume: bool,
    pub b_lose_limb_cheat: bool,
    pub b_switched_teams_in_loadout_volume: bool,
    pub _pad_after_flags: [u8; 0x2],                 // 0x002E
    pub hit_result: [u8; 0x88],                      // 0x0030
    pub ability_spec: *mut c_void,                   // 0x00B8
    pub abilities_table_row: crate::chivalry2::engine::FName,       // 0x00C0
    pub hit_direction: crate::chivalry2::engine::FVector,           // 0x00C8 (FVector_NetQuantizeNormal)
    pub damage_taker_combat_state: crate::chivalry2::engine::FName, // 0x00D4
    pub apply_condition: u8,                         // EConditionType, 0x00DC
    pub _pad_after_apply_condition: [u8; 0x3],       // 0x00DD
    pub post_damage_info: [u8; 0x0C],                // 0x00E0
    pub _pad_before_inventory_item: [u8; 0x4],       // 0x00EC
    pub inventory_item: *mut c_void,                 // 0x00F0
    pub projectile: *mut c_void,                     // 0x00F8
    pub location_based_damage: u8,                   // ELocationBasedDamage, 0x0100
    pub _pad_after_location_based_damage: [u8; 0x7], // 0x0101
    pub attach_parent: *mut c_void,                  // 0x0108
    pub b_parried: bool,                             // 0x0110
    pub _pad_after_b_parried: [u8; 0x7],             // 0x0111
    pub character_who_parried: *mut c_void,          // 0x0118
    pub b_is_in_team_thwack_range: bool,             // 0x0120
    pub _pad_before_gore_event: [u8; 0x3],           // 0x0121
    pub gore_event: [u8; 0x1C],                      // 0x0124
    pub kill_reason: EKillReason,                    // 0x0140
    pub b_arrow_parried: bool,                       // 0x0141
    pub b_disarmed: bool,                            // 0x0142
    pub _pad_end: [u8; 0x5],                         // 0x0143
}

const _: [(); 0x148] = [(); std::mem::size_of::<FDamageTakenEvent>()];
