//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/UObject/SoftObjectPtr.h>.

use crate::Chivalry2::Engine::*;

use std::os::raw::c_void;

#[repr(C)]
#[derive(Debug)]
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

const _: () = assert!(std::mem::size_of::<FSoftObjectPtr>() == 0x28);

#[repr(C)]
#[derive(Debug)]
pub struct TSoftClassPtr<T = c_void> {
    pub soft_ptr: FSoftObjectPtr,
    _marker: std::marker::PhantomData<T>,
}

impl<T> TSoftClassPtr<T> {
    pub fn from_path(path: &str) -> Self {
        Self {
            soft_ptr: FSoftObjectPtr::from_path(path),
            _marker: std::marker::PhantomData,
        }
    }
}
