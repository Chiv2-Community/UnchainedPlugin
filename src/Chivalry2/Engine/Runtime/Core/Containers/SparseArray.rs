//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/Containers/SparseArray.h>.

#![allow(private_interfaces)]
use crate::Chivalry2::Engine::*;

use std::fmt::Debug;

use crate::Chivalry2::Engine::TArray;

#[unchained_macros_core::runtime_bindings]
#[repr(C)]
pub(crate) union TSparseArrayElementOrFreeListLink<E> {
    pub(crate) element: std::mem::ManuallyDrop<E>,
    pub(crate) list_link: ListLink,
}

#[unchained_macros_core::runtime_bindings]
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub(crate) struct ListLink {
    pub(crate) next_free_index: i32,
    pub(crate) prev_free_index: i32,
}

#[unchained_macros_core::runtime_bindings]
#[repr(C)]
pub(crate) struct TSparseArray<E> {
    pub data: TArray<TSparseArrayElementOrFreeListLink<E>>,
    pub(crate) allocation_flags: TBitArray,
    pub(crate) first_free_index: i32,
    pub(crate) num_free_indices: i32,
}

impl<E> TSparseArray<E> {
    pub(crate) fn index(&self, index: usize) -> &E {
        assert!(index < self.data.len() && index < self.allocation_flags.num_bits as usize);
        assert!(self.allocation_flags.index(index).bool());
        unsafe { &self.data.as_slice()[index].element }
    }
}

impl<E> Default for TSparseArray<E> {
    fn default() -> Self {
        Self {
            data: Default::default(),
            allocation_flags: Default::default(),
            first_free_index: 0,
            num_free_indices: 0,
        }
    }
}

pub(crate) struct DbgTSparseArrayData<'a, E>(&'a TSparseArray<E>);

impl<E: Debug> Debug for DbgTSparseArrayData<'_, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut dbg = f.debug_list();
        for i in 0..(self.0.allocation_flags.num_bits as usize) {
            if self.0.allocation_flags.index(i).bool() {
                dbg.entry(self.0.index(i));
            }
        }
        dbg.finish()
    }
}

impl<E: Debug> Debug for TSparseArray<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TSparseArray")
            .field("data", &DbgTSparseArrayData(self))
            .field("allocation_flags", &self.allocation_flags)
            .field("first_free_index", &self.first_free_index)
            .field("num_free_indices", &self.num_free_indices)
            .finish()
    }
}
