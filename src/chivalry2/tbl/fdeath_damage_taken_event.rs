//! SDK declaration: <https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/DeathDamageTakenEvent.h>.
//! Shipping PDB attributes this type to StatsComponent.generated.h; original header ownership is unresolved.

#![allow(non_snake_case)]
use crate::chivalry2::engine::*;
use crate::chivalry2::tbl::*;
use std::os::raw::c_void;

/// Layout based on `CXXHeaderDump/TBL.hpp` (`Size: 0x160`).
#[repr(C)]
#[derive(Debug)]
pub struct FDeathDamageTakenEvent {
    pub damage_taken: FDamageTakenEvent,
    pub killers: TArray<*mut c_void>, // 0x0148
    pub random_seed: i32,             // 0x0158
    pub kill_reason: EKillReason,     // 0x015C
    pub dead_character_id: u8,        // 0x015D
    pub b_attach_to_projectile: bool, // 0x015E
    pub _pad_end: [u8; 0x1],          // 0x015F
}

const _: [(); 0x160] = [(); std::mem::size_of::<FDeathDamageTakenEvent>()];
