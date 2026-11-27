//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/Containers/ContainerAllocationPolicies.h>.

#![allow(private_interfaces)]

use std::fmt::Debug;

#[unchained_macros_core::runtime_bindings]
#[derive(Debug)]
#[repr(C)]
pub(crate) struct TInlineAllocator<const N: usize, V> {
    pub(crate) inline_data: [V; N],
    pub(crate) secondary_data: *const V, // TSizedHeapAllocator<32>::ForElementType<unsigned int>,
}

impl<const N: usize, V> TInlineAllocator<N, V> {
    pub(crate) fn get_allocation(&self) -> *const V {
        if !self.secondary_data.is_null() {
            self.secondary_data
        } else {
            self.inline_data.as_ptr()
        }
    }
}

impl<const N: usize, V: Default + Copy> Default for TInlineAllocator<N, V> {
    fn default() -> Self {
        Self {
            inline_data: [Default::default(); N],
            secondary_data: std::ptr::null(),
        }
    }
}
