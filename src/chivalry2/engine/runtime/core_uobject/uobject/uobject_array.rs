//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/UObject/UObjectArray.h>.

use crate::chivalry2::engine::runtime::core::windows::windows_critical_section::CriticalSectionGuard;
use crate::chivalry2::engine::runtime::core::windows::windows_critical_section::FWindowsCriticalSection;
use crate::chivalry2::engine::TArray;
use crate::chivalry2::engine::UObjectBase;
use crate::chivalry2::engine::*;
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicI32, Ordering};

type ObjectIndex = i32;

#[derive(Debug)]
#[repr(C)]
pub struct FUObjectCreateListener;

#[derive(Debug)]
#[repr(C)]
pub struct FUObjectDeleteListener;

#[derive(Debug)]
#[repr(C)]
pub struct FUObjectArray {
    obj_first_gcindex: i32,
    obj_last_non_gcindex: i32,
    max_objects_not_considered_by_gc: i32,
    open_for_disregard_for_gc: bool,

    obj_objects: UnsafeCell<FChunkedFixedUObjectArray>,
    obj_objects_critical: FWindowsCriticalSection,
    obj_available_list: [u8; 0x88],
    uobject_create_listeners: TArray<*const FUObjectCreateListener>,
    uobject_delete_listeners: TArray<*const FUObjectDeleteListener>,
    uobject_delete_listeners_critical: FWindowsCriticalSection,
    master_serial_number: AtomicI32,
}

impl FUObjectArray {
    /// Address of UE's inline object registry; never copy its native storage.
    ///
    /// # Safety
    /// Startup must bind the process-lifetime FUObjectArray. Access through the
    /// returned pointer must follow the registry's native synchronization rules.
    #[unchained_macros::bind_resolver_value(
        patternsleuth::resolvers::unreal::guobject_array::GUObjectArray,
        Address
    )]
    pub(crate) unsafe fn native_guobject_array() -> *const FUObjectArray {}

    pub fn objects(&self) -> CriticalSectionGuard<'_, '_, FChunkedFixedUObjectArray> {
        CriticalSectionGuard::lock(&self.obj_objects_critical, &self.obj_objects)
    }

    /// True when `obj` is a live object: the InternalIndex stored in it, read without faulting,
    /// names a slot that still holds this very pointer, and that slot is neither pending kill nor
    /// unreachable. A dangling or garbage pointer fails because its memory is unreadable, or its
    /// slot is out of range, empty or holds another object. Reads the chunk table without the
    /// lock, as allocate_serial_number below does; meant for the game thread.
    pub fn is_live(&self, obj: *const UObjectBase) -> bool {
        const UNREACHABLE: i32 = 1 << 28;
        const PENDING_KILL: i32 = 1 << 29;
        if obj.is_null() {
            return false;
        }
        let index_ptr = (obj as *const u8)
            .wrapping_add(std::mem::offset_of!(UObjectBase, internal_index))
            as *const i32;
        let Some(index) = FMemory::try_read(index_ptr) else {
            return false;
        };
        let objects = unsafe { &*self.obj_objects.get() };
        if index < 0 || index >= objects.num_elements || objects.max_chunks <= 0 {
            return false;
        }
        let item = objects.item(index);
        std::ptr::eq(item.object, obj) && item.flags & (UNREACHABLE | PENDING_KILL) == 0
    }

    pub fn allocate_serial_number(&self, index: ObjectIndex) -> i32 {
        let objects = unsafe { &*self.obj_objects.get() };
        let item = objects.item(index);

        let current = item.serial_number.load(Ordering::SeqCst);
        if current != 0 {
            current
        } else {
            let new = self.master_serial_number.fetch_add(1, Ordering::SeqCst);

            let exchange =
                item.serial_number
                    .compare_exchange(0, new, Ordering::SeqCst, Ordering::SeqCst);
            match exchange {
                Ok(_) => new,
                Err(old) => old,
            }
        }
    }
}

pub struct ObjectIterator<'a> {
    array: &'a FChunkedFixedUObjectArray,
    index: i32,
}

impl<'a> Iterator for ObjectIterator<'a> {
    type Item = Option<&'a UObjectBase>;
    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = self.array.num_elements as usize;
        (size, Some(size))
    }
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let n = n as i32;
        if self.index < n {
            self.index = n;
        }
        self.next()
    }
    fn next(&mut self) -> Option<Option<&'a UObjectBase>> {
        if self.index >= self.array.num_elements {
            None
        } else {
            let obj = unsafe { self.array.item(self.index).object.as_ref() };

            self.index += 1;
            Some(obj)
        }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct FChunkedFixedUObjectArray {
    pub objects: *const *const FUObjectItem,
    pub pre_allocated_objects: *const FUObjectItem,
    pub max_elements: i32,
    pub num_elements: i32,
    pub max_chunks: i32,
    pub num_chunks: i32,
}

impl FChunkedFixedUObjectArray {
    pub fn iter(&self) -> ObjectIterator<'_> {
        ObjectIterator {
            array: self,
            index: 0,
        }
    }
    fn item_ptr(&self, index: ObjectIndex) -> *const FUObjectItem {
        let per_chunk = self.max_elements / self.max_chunks;

        unsafe {
            (*self.objects.add((index / per_chunk) as usize)).add((index % per_chunk) as usize)
        }
    }
    pub fn item(&self, index: ObjectIndex) -> &FUObjectItem {
        unsafe { &*self.item_ptr(index) }
    }
    pub fn item_mut(&mut self, index: ObjectIndex) -> &mut FUObjectItem {
        unsafe { &mut *(self.item_ptr(index) as *mut FUObjectItem) }
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct FUObjectItem {
    pub object: *const UObjectBase,
    pub flags: i32,
    pub cluster_root_index: i32,
    pub serial_number: AtomicI32,
}

#[cfg(test)]
mod tests {
    #[test]
    #[should_panic(expected = "Required native value GUObjectArray is not bound")]
    fn object_registry_access_requires_binding() {
        unsafe {
            super::FUObjectArray::native_guobject_array();
        }
    }
}
