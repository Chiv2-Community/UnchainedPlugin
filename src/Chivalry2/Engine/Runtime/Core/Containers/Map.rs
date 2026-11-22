//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/Containers/Map.h>.

#![allow(private_interfaces)]
use crate::Chivalry2::Engine::*;

use std::fmt::Debug;

use std::collections::HashMap;

#[unchained_macros_core::runtime_bindings]
#[repr(C)]
pub struct TMap<K: UEHash, V> {
    pub elements: TSparseArray<TSetElement<TTuple<K, V>>>,
    pub(crate) hash: TInlineAllocator<1, FSetElementId>,
    pub(crate) hash_size: i32,
}

impl<K: UEHash, V> TMap<K, V> {
    pub fn to_hashmap(&self) -> HashMap<K, V>
    where
        K: PartialEq + std::hash::Hash + Eq + Clone,
        V: Clone,
    {
        self.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
    }

    pub fn into_hashmap(self) -> HashMap<K, V>
    where
        K: Eq + std::hash::Hash,
    {
        let mut result = HashMap::new();
        for i in 0..self.elements.data.len() {
            if self.elements.allocation_flags.index(i).bool() {
                let elem = unsafe { std::ptr::read(self.elements.index(i)) };
                let TTuple { a, b } = elem.value;
                result.insert(a, b);
            }
        }
        result
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)>
    where
        K: PartialEq,
    {
        let elements = &self.elements;
        (0..elements.data.len())
            .filter(move |&i| elements.allocation_flags.index(i).bool())
            .map(move |i| {
                let elem = elements.index(i);
                (&elem.value.a, &elem.value.b)
            })
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&K, &mut V)>
    where
        K: PartialEq,
    {
        let allocation_flags = &self.elements.allocation_flags;
        let data = self.elements.data.as_mut_slice();

        data.iter_mut().enumerate().filter_map(move |(i, entry)| {
            if allocation_flags.index(i).bool() {
                // SAFETY: The bit is set, so this is a live element.
                let elem_ptr = unsafe { &mut entry.element };
                let tuple = &mut elem_ptr.value;

                Some((&tuple.a, &mut tuple.b))
            } else {
                None
            }
        })
    }

    pub fn find(&self, key: K) -> Option<&V>
    where
        K: PartialEq,
    {
        let id = self.find_id(key);
        if id.is_valid() {
            Some(&self.elements.index(id.index as usize).value.b)
        } else {
            None
        }
    }
    pub fn find_id(&self, key: K) -> FSetElementId
    where
        K: PartialEq,
    {
        if self.elements.data.len() != self.elements.num_free_indices as usize {
            let key_hash = key.ue_hash();
            let hash = &self.hash();

            let mut i: FSetElementId =
                hash[(((self.hash_size as i64) - 1) & (key_hash as i64)) as usize];

            if i.is_valid() {
                loop {
                    let elm = self.elements.index(i.index as usize);

                    if elm.value.a == key {
                        return i;
                    }

                    i = elm.hash_next_id;
                    if !i.is_valid() {
                        break;
                    }
                }
            }
        }

        FSetElementId { index: -1 }
    }

    fn hash(&self) -> &[FSetElementId] {
        unsafe { std::slice::from_raw_parts(self.hash.get_allocation(), self.hash_size as usize) }
    }
}

impl<K: UEHash, V> Default for TMap<K, V> {
    fn default() -> Self {
        Self {
            elements: Default::default(),
            hash: Default::default(),
            hash_size: 0,
        }
    }
}

impl<K: UEHash + Debug, V: Debug> Debug for TMap<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TMap")
            .field("elements", &self.elements)
            .field("hash", &self.hash())
            .finish()
    }
}

/// Read-only view of an engine `TMap<K, V>` for key types the plugin cannot
/// hash the way the engine does. Iteration only; same layout as [`TMap`].
#[repr(C)]
pub struct TMapView<K, V> {
    pub(crate) pairs: TSet<TTuple<K, V>>,
}

impl<K, V> TMapView<K, V> {
    /// Iterates the live key/value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.pairs.iter().map(|pair| (&pair.a, &pair.b))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn empty_tset_iterates_nothing() {
        // An all-zero TSet is what an empty Blueprint set property looks like.
        let set: TSet<u64> = unsafe { std::mem::zeroed() };
        assert_eq!(set.iter().count(), 0);
        assert!(set.is_empty());
        std::mem::forget(set);
    }
}
