//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/Templates/TypeHash.h>.

#![allow(non_snake_case)]
use crate::chivalry2::engine::*;
use std::hash::{DefaultHasher, Hash, Hasher};

impl UEHash for *mut crate::chivalry2::engine::UClass {
    fn ue_hash(&self) -> u32 {
        let mut hasher = DefaultHasher::new();
        (*self as usize).hash(&mut hasher);
        (hasher.finish() & 0xFFFF_FFFF) as u32
    }
}

impl UEHash for *mut crate::chivalry2::engine::UObject {
    fn ue_hash(&self) -> u32 {
        let mut hasher = DefaultHasher::new();
        (*self as usize).hash(&mut hasher);
        (hasher.finish() & 0xFFFF_FFFF) as u32
    }
}
