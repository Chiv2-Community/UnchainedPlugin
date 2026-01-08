#![allow(dead_code)]
#![allow(non_camel_case_types)]
#![allow(clippy::upper_case_acronyms)]

use std::hash::{DefaultHasher, Hash, Hasher};
use std::os::raw::c_void;
use widestring::U16CString;

use crate::resolvers::asset_registry::{FAssetData, TScriptInterface, o_FNameCtorWchar};
use crate::sinfo;
use crate::ue::{EFindName, EObjectFlags, FName, FNameEntryId, FString, TArray, UObject};
use crate::resolvers::asset_registry::*;
// use crate::resolvers::asset_registry::o_StaticFindObject;
// use crate::resolvers::asset_registry::o_StaticLoadObject;
// use crate::resolvers::asset_registry::o_GetAssetRegistry_Helper;
// use crate::resolvers::asset_registry::o_GetAssetsByClass;

#[repr(C)]
pub enum ENetMode {
    STANDALONE,
    DEDICATED_SERVER,
    LISTEN_SERVER,
    CLIENT,
    MAX,
}

// Hashes for TMap
use crate::ue::UEHash;
impl UEHash for *mut crate::ue::UClass {
    fn ue_hash(&self) -> u32 {
        let mut hasher = DefaultHasher::new();
        (*self as usize).hash(&mut hasher);
        (hasher.finish() & 0xFFFF_FFFF) as u32
    }
}
impl UEHash for *mut crate::ue::UObject {
    fn ue_hash(&self) -> u32 {
        let mut hasher = DefaultHasher::new();
        (*self as usize).hash(&mut hasher);
        (hasher.finish() & 0xFFFF_FFFF) as u32
    }
}
pub struct FText
{
    pub text_data: [u8; 0x10],
    pub flags: u32
}
impl Default for FText {
    fn default() -> Self {
        Self {
            text_data: [0u8; 0x10],
            flags: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct FRotator {
    /// Rotation around the right axis (Look up/down), 0=Horizontal, +Up, -Down.
    pub pitch: f32, // 0x0
    /// Rotation around the up axis (Turn left/right), 0=Forward, +Right, -Left.
    pub yaw: f32,   // 0x4
    /// Rotation around the forward axis (Tilt left/right), 0=No tilt, +Clockwise, -CCW.
    pub roll: f32,  // 0x8
}

impl FRotator {
    pub const ZERO: Self = Self { pitch: 0.0, yaw: 0.0, roll: 0.0 };

    pub fn new(pitch: f32, yaw: f32, roll: f32) -> Self {
        Self { pitch, yaw, roll }
    }

    /// Returns a vector pointing in the direction of this rotation
    pub fn to_vector(&self) -> [f32; 3] {
        let p = self.pitch.to_radians();
        let y = self.yaw.to_radians();
        
        let cp = p.cos();
        let sp = p.sin();
        let cy = y.cos();
        let sy = y.sin();

        [cp * cy, cp * sy, sp]
    }
}

// SoftClassPath

#[repr(C)]
pub struct FSoftObjectPath {
    pub asset_path_name: FName,   // 0x00
    pub sub_path_string: FString, // 0x08
}

impl FSoftObjectPath {
    pub fn new(path: &str) -> Self {
        Self {
            // We use 'Find' for existing assets, or 'Add' if you are creating a path
            asset_path_name: FName::with_type(path, EFindName::Find),
            sub_path_string: FString::default(),
        }
    }
}

#[repr(C)]
pub struct FSoftObjectPtr {
    pub weak_ptr: [u8; 8],     // 0x00
    pub tag_at_last_test: i32, // 0x08
    pub padding: u32,          // 0x0C

    pub path: FSoftObjectPath, // 0x10
}

impl FSoftObjectPtr {
    pub fn from_path(path: &str) -> Self {
        Self {
            // WeakPtr must be zeroed so the engine knows the cache is cold
            weak_ptr: [0u8; 8],
            tag_at_last_test: -1,
            padding: 0,
            path: FSoftObjectPath::new(path),
        }
    }
}

impl<T> TSoftClassPtr<T> {
    pub fn from_path(path: &str) -> Self {
        Self {
            soft_ptr: FSoftObjectPtr::from_path(path),
            _marker: std::marker::PhantomData,
        }
    }
}

#[repr(C)]
pub struct TSoftClassPtr<T = c_void> {
    pub soft_ptr: FSoftObjectPtr,
    _marker: std::marker::PhantomData<T>,
}

// FName helpers

impl FName {
    pub fn with_type(path: &str, find_type: EFindName) -> Self {
        let wide_path = widestring::U16CString::from_str(path).unwrap();
        let mut name = FName::default();
		sinfo!(f; "Constructing fname for {}", path);

		CALL_ORIGINAL_SAFE!(FNameCtorWchar(
			&mut name as *mut FName,
			wide_path.as_ptr(),
			find_type
		))
		.expect("Failed to construct FName");
		if name.comparison_index.value == 0 {
			sinfo!(f; "1. Name is invalid for {}", wide_path.to_string_lossy());
			CALL_ORIGINAL_SAFE!(FNameCtorWchar(
				&mut name as *mut FName,
				wide_path.as_ptr(),
				EFindName::Add
			))
			.expect("Failed to construct FName");
		}
		if name.comparison_index.value == 0 {
			sinfo!(f; "2. Name is invalid for {}", wide_path.to_string_lossy());
		}
        name
    }
}


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
    pub template: *const c_void,   // AActor*
    pub owner: *const c_void,      // AActor*
    pub instigator: *const c_void, // APawn*
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
    pub fn remote_owned(&self) -> bool { (self.bitfield & (1 << 0)) != 0 }
    // pub fn no_fail(&self) -> bool { (self.bitfield & (1 << 1)) != 0 }
    pub fn defer_construction(&self) -> bool { (self.bitfield & (1 << 2)) != 0 }
    pub fn allow_during_construction(&self) -> bool { (self.bitfield & (1 << 3)) != 0 }
}

// Helper functions

pub unsafe fn get_uobject_from_path(path: &str, load_if_missing: bool) -> *mut UObject {
    let wide_path = U16CString::from_str(path).unwrap();
    
    if !load_if_missing {
        // Try to find it in memory first
        return CALL_ORIGINAL!(StaticFindObject(
            std::ptr::null_mut(), 
            std::ptr::null_mut(), 
            wide_path.as_ptr(), 
            false
        ));
    }

    CALL_ORIGINAL_SAFE!(StaticLoadObject(
        std::ptr::null_mut(), // Any class
        std::ptr::null_mut(), // No specific outer
        wide_path.as_ptr(), 
        std::ptr::null(),     // Filename (usually null)
        0,                    // LoadFlags (0 = LoadConfig)
        std::ptr::null_mut(), // Sandbox
        false,                // AllowNativeComponentClass
        std::ptr::null_mut()
    )).expect("Failed to find object")
}

pub unsafe fn get_assets_by_class(class_name: String) -> Result<TArray<FAssetData>, String> {
    let mut asset_registry = TScriptInterface::new();
    let _: *mut TScriptInterface = CALL_ORIGINAL_SAFE!(GetAssetRegistry_Helper(
        &mut asset_registry as *mut TScriptInterface
    ))
    .expect("GetAssetRegistry_Helper failed");

    let name_res = FName::with_type(class_name.as_str(), crate::ue::EFindName::Find);

    let asset_registry_interface = {
        if asset_registry.interface.is_null() {
            // Fallback: If the interface pointer isn't set,
            // sometimes the Object itself IS the interface.
            crate::serror!("interface is null");
            asset_registry.object as *mut c_void
        } else {
            asset_registry.interface as *mut c_void
        }
    };

    let mut asset_data = TArray::<FAssetData>::default();
    let out = CALL_ORIGINAL_SAFE!(GetAssetsByClass(
        asset_registry_interface,
        name_res,
        &mut asset_data as *mut TArray<FAssetData>,
        true
    ))
    .expect("GetAssetsByClass failed");

    match out {
        true => Ok(asset_data),
        false => Err("".into()),
    }
}

