//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/UObject/WeakObjectPtr.h>.

use crate::Chivalry2::Engine::*;

use std::ops::Deref;

use std::sync::atomic::Ordering;

use crate::Chivalry2::Engine::Runtime::CoreUObject::UObject::UObjectArray::FUObjectArray;

#[unchained_macros_core::runtime_bindings]
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FWeakObjectPtr {
    object_index: i32,
    object_serial_number: i32,
}

impl FWeakObjectPtr {
    pub fn new(object: &UObjectBase) -> Self {
        Self::new_from_index(object.internal_index)
    }
    pub fn new_from_index(index: i32) -> Self {
        Self {
            object_index: index,
            // serial allocation performs only atomic operations
            object_serial_number: unsafe {
                GUObjectArray.allocate_serial_number(index)
            },
        }
    }
    pub unsafe fn get<'a>(&self, object_array: &'a FUObjectArray) -> Option<&'a UObjectBase> {
        let guard = object_array.objects();
        let objects = guard.deref();
        if self.object_index < 0 || self.object_index >= objects.num_elements {
            return None;
        }
        let item = objects.item(self.object_index);
        if item.serial_number.load(Ordering::Relaxed) != self.object_serial_number {
            return None;
        }
        item.object.as_ref()
    }
}
