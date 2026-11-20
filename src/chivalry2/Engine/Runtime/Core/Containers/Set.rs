//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/Containers/Set.h>.

#![allow(private_interfaces)]
use crate::chivalry2::Engine::*;

use std::fmt::Debug;

#[derive(Default, Debug)]
#[repr(C)]
pub(crate) struct TSetElement<V> {
    pub(crate) value: V,
    pub(crate) hash_next_id: FSetElementId,
    pub(crate) hash_index: i32,
}

impl<K: UEHash, V> UEHash for TSetElement<TTuple<K, V>> {
    fn ue_hash(&self) -> u32 {
        self.value.a.ue_hash()
    }
}

#[derive(Default, Clone, Copy)]
#[repr(C)]
pub struct FSetElementId {
    pub(crate) index: i32,
}

impl FSetElementId {
    pub fn is_valid(self) -> bool {
        self.index != -1
    }
}

impl Debug for FSetElementId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FSetElementId({:?})", self.index)
    }
}

/// Read-only view of an engine `TSet<T>`. Only ever used through a reference
/// into live engine memory; Rust never constructs, mutates or drops one.
#[repr(C)]
pub struct TSet<T> {
    pub(crate) elements: TSparseArray<TSetElement<T>>,
    pub(crate) hash: TInlineAllocator<1, FSetElementId>,
    pub(crate) hash_size: i32,
}

impl<T> TSet<T> {
    /// Iterates the live elements (allocation order, free slots skipped).
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        let elements = &self.elements;
        let slots = elements
            .data
            .len()
            .min(elements.allocation_flags.num_bits.max(0) as usize);
        (0..slots)
            .filter(move |&i| elements.allocation_flags.index(i).bool())
            .map(move |i| &elements.index(i).value)
    }

    /// Number of live elements.
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
