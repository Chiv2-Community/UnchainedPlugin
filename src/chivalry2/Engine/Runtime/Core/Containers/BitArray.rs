//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/Containers/BitArray.h>.

#![allow(private_interfaces)]
use crate::chivalry2::Engine::*;

use std::fmt::Debug;

#[derive(Default, Debug)]
#[repr(C)]
pub(crate) struct TBitArray {
    pub(crate) allocator_instance: TInlineAllocator<4, u32>,
    pub(crate) num_bits: i32,
    pub(crate) max_bits: i32,
}

impl TBitArray {
    pub(crate) fn get_data(&self) -> *const u32 {
        self.allocator_instance.get_allocation()
    }

    pub(crate) fn index(&self, index: usize) -> FBitReference<'_> {
        assert!(index < self.num_bits as usize);
        let num_bits_per_dword = 32;
        FBitReference {
            data: unsafe { &*self.get_data().add(index / num_bits_per_dword) },
            mask: 1 << (index & (num_bits_per_dword - 1)),
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub(crate) struct FBitReference<'data> {
    pub(crate) data: &'data u32,
    pub(crate) mask: u32,
}

impl FBitReference<'_> {
    pub(crate) fn bool(self) -> bool {
        (self.data & self.mask) != 0
    }
}
