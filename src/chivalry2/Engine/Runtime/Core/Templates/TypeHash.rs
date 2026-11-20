//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/Templates/TypeHash.h>.

use std::hash::{DefaultHasher, Hash, Hasher};

pub trait UEHash {
    fn ue_hash(&self) -> u32;
}

impl UEHash for *mut crate::chivalry2::Engine::UClass {
    fn ue_hash(&self) -> u32 {
        let mut hasher = DefaultHasher::new();
        (*self as usize).hash(&mut hasher);
        (hasher.finish() & 0xFFFF_FFFF) as u32
    }
}

impl UEHash for *mut crate::chivalry2::Engine::UObject {
    fn ue_hash(&self) -> u32 {
        let mut hasher = DefaultHasher::new();
        (*self as usize).hash(&mut hasher);
        (hasher.finish() & 0xFFFF_FFFF) as u32
    }
}
