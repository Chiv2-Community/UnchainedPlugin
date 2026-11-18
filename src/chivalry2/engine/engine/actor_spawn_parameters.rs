//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/Engine/World.h>.

#![allow(non_snake_case)]
use crate::chivalry2::engine::*;
use std::os::raw::c_void;

// FName helpers

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ESpawnActorCollisionHandlingMethod {
    Undefined = 0,
    AlwaysSpawn = 1,
    AdjustIfPossibleButAlwaysSpawn = 2,
    AdjustIfPossibleButDontSpawnIfColliding = 3,
    DontSpawnIfColliding = 4,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ESpawnActorNameMode {
    RequiredFatal = 0,
    RequiredErrorAndReturnNull = 1,
    RequiredReturnNull = 2,
    Requested = 3,
}

// Spawning actors
#[repr(C, align(8))]
pub struct FActorSpawnParameters {
    // 0x00: FName (Name) - Size 0x8
    pub name: u64,
    // 0x08 - 0x20: Pointer types
    pub template: *const c_void,       // AActor*
    pub owner: *const c_void,          // AActor*
    pub instigator: *const c_void,     // APawn*
    pub override_level: *const c_void, // ULevel*
    // 0x28: Size 0x1
    pub spawn_collision_handling_override: ESpawnActorCollisionHandlingMethod, // ESpawnActorCollisionHandlingMethod
    // 0x29: Bitfield uchar:1 (Size 0x1)
    // Offset 0x29 handles 4 bits: bRemoteOwned, bNoFail, bDeferConstruction, bAllowDuringConstructionScript
    pub bitfield: u8,
    // 0x2a: Size 0x1
    pub name_mode: ESpawnActorNameMode, // ESpawnActorNameMode
    // 0x2b: Padding
    pub _padding: u8,
    // 0x2c: Size 0x4
    pub object_flags: EObjectFlags, // EObjectFlags
}

impl Default for FActorSpawnParameters {
    fn default() -> Self {
        Self {
            name: 0, // None
            template: std::ptr::null(),
            owner: std::ptr::null(),
            instigator: std::ptr::null(),
            override_level: std::ptr::null(),
            // Default UE4/5 behavior: AlwaysSpawn
            spawn_collision_handling_override: ESpawnActorCollisionHandlingMethod::AlwaysSpawn,
            bitfield: 0,
            // Default UE4/5 behavior: Requested
            name_mode: ESpawnActorNameMode::Requested,
            _padding: 0,
            // Default UE4/5 behavior: RF_NoFlags
            object_flags: EObjectFlags::RF_NoFlags,
        }
    }
}

impl FActorSpawnParameters {
    pub fn new() -> Self {
        Self::default()
    }
    /// Builder method to set the Spawn Collision Handling mode
    pub fn with_spawn_mode(mut self, mode: ESpawnActorCollisionHandlingMethod) -> Self {
        self.spawn_collision_handling_override = mode;
        self
    }
    /// Builder method to set the Owner
    pub fn with_owner(mut self, owner: *const c_void) -> Self {
        self.owner = owner;
        self
    }
    /// Builder method to set the bNoFail bit
    pub fn no_fail(mut self, enabled: bool) -> Self {
        if enabled {
            self.bitfield |= 0x02;
        } else {
            self.bitfield &= !0x02;
        }
        self
    }
    pub fn remote_owned(&self) -> bool {
        (self.bitfield & (1 << 0)) != 0
    }
    pub fn defer_construction(&self) -> bool {
        (self.bitfield & (1 << 2)) != 0
    }
    pub fn allow_during_construction(&self) -> bool {
        (self.bitfield & (1 << 3)) != 0
    }
}
