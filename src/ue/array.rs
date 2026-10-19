use std::alloc::Layout;

use crate::globals;

#[derive(Debug, Clone)]
#[repr(C)]
pub struct TArray<T> {
    data: *mut T,
    num: i32,
    max: i32,
}
impl<T> TArray<T> {
    /// Append before plugin globals exist, using the engine's allocator.
    ///
    /// # Safety
    /// `allocator` must be the engine allocator that owns this array's buffer.
    pub(crate) unsafe fn push_in(&mut self, value: T, allocator: super::FMallocHandle) {
        let required = self.num.checked_add(1).expect("TArray length overflow");
        if required > self.max {
            self.grow_in(required as usize, allocator);
        }
        self.push_reserved(value);
    }

    /// Copy a buffer before plugin globals exist.
    ///
    /// # Safety
    /// `allocator` must be the engine allocator. Transfer the result to the engine
    /// before dropping it, since `Drop` uses plugin globals.
    pub(crate) unsafe fn from_slice_in(value: &[T], allocator: super::FMallocHandle) -> Self
    where
        T: Copy,
    {
        if value.is_empty() {
            return Self::new();
        }
        let mut array = Self::with_capacity_in(value.len(), allocator);
        std::ptr::copy_nonoverlapping(value.as_ptr(), array.data, value.len());
        array.num = array.max;
        array
    }

    /// # Safety
    /// `allocator` must own the returned buffer. Transfer it to the engine before
    /// dropping it if plugin globals are not initialized.
    pub(crate) unsafe fn with_capacity_in(capacity: usize, allocator: super::FMallocHandle) -> Self {
        if capacity == 0 {
            return Self::new();
        }
        let max = i32::try_from(capacity).expect("TArray capacity overflow");
        let layout = Layout::array::<T>(capacity).expect("Layout error");
        let data = allocator.malloc(layout).cast::<T>();
        if data.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        Self { data, num: 0, max }
    }

    unsafe fn grow_in(&mut self, capacity: usize, allocator: super::FMallocHandle) {
        let max = i32::try_from(capacity).expect("TArray capacity overflow");
        let layout = Layout::array::<T>(capacity).expect("Layout error");
        let data = allocator.realloc(self.data.cast(), layout).cast::<T>();
        if data.is_null() {
            std::alloc::handle_alloc_error(layout);
        }
        self.data = data;
        self.max = max;
    }

    /// # Safety
    /// `allocator` must own this array's buffer.
    pub(crate) unsafe fn reserve_in(&mut self, additional: usize, allocator: super::FMallocHandle) {
        let required = self.len().checked_add(additional).expect("TArray length overflow");
        if required > self.capacity() {
            let capacity = required.checked_next_power_of_two().expect("TArray capacity overflow");
            self.grow_in(capacity, allocator);
        }
    }

    unsafe fn push_reserved(&mut self, value: T) {
        std::ptr::write(self.data.add(self.num as usize), value);
        self.num += 1;
    }

    pub fn new() -> Self {
        Self {
            data: std::ptr::null_mut(),
            num: 0,
            max: 0,
        }
    }
    pub fn as_ptr(&self) -> *const T {
        self.data
    }
    pub fn get(&self, index: usize) -> Option<&T> {
        if index < self.len() {
            Some(unsafe { &*self.data.add(index) })
        } else {
            None
        }
    }
    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index < self.len() {
            Some(unsafe { &mut *self.data.add(index) })
        } else {
            None
        }
    }
    pub fn with_capacity(capacity: usize) -> Self {
        if capacity == 0 {
            return Self::new();
        }
        unsafe {
            Self::with_capacity_in(capacity, super::try_gmalloc().expect("GMalloc is not initialized"))
        }
    }
    pub fn len(&self) -> usize {
        self.num.max(0) as usize
    }
    pub fn capacity(&self) -> usize {
        self.max.max(0) as usize
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn as_slice(&self) -> &[T] {
        if self.num <= 0 || self.data.is_null() || (self.data as usize % std::mem::align_of::<T>()) != 0 {
            &[]
        } else {
            let num = self.num as usize;
            let size = std::mem::size_of::<T>();
            // If T is a ZST, size is 0 and we only check align and null (already done above)
            if size > 0 && num > (isize::MAX as usize / size) {
                &[]
            } else {
                unsafe { std::slice::from_raw_parts(self.data, num) }
            }
        }
    }
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        if self.num <= 0 || self.data.is_null() || (self.data as usize % std::mem::align_of::<T>()) != 0 {
            &mut []
        } else {
            let num = self.num as usize;
            let size = std::mem::size_of::<T>();
            // If T is a ZST, size is 0 and we only check align and null (already done above)
            if size > 0 && num > (isize::MAX as usize / size) {
                &mut []
            } else {
                unsafe { std::slice::from_raw_parts_mut(self.data as *mut _, num) }
            }
        }
    }
    pub fn clear(&mut self) {
        let elems: *mut [T] = self.as_mut_slice();

        unsafe {
            self.num = 0;
            std::ptr::drop_in_place(elems);
        }
    }
    pub fn reserve(&mut self, additional: usize) {
        let required_cap = self.len().checked_add(additional).expect("TArray length overflow");
        if required_cap > self.capacity() {
            unsafe {
                self.reserve_in(additional, super::try_gmalloc().expect("GMalloc is not initialized"))
            };
        }
    }
    pub fn push(&mut self, new_value: T) {
        self.reserve(1);
        unsafe {
            self.push_reserved(new_value);
        }
    }
    pub fn extend_from_slice(&mut self, other: &[T])
    where
        T: Copy,
    {
        self.reserve(other.len());
        // SAFETY: reserve ensures we have enough space. 
        // We use copy_nonoverlapping for Copy types for better performance.
        unsafe {
            std::ptr::copy_nonoverlapping(
                other.as_ptr(),
                self.data.add(self.num as usize),
                other.len(),
            );
            self.num += other.len() as i32;
        }
    }
}
impl<T> std::ops::Index<usize> for TArray<T> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).expect("TArray index out of bounds")
    }
}
impl<T> std::ops::IndexMut<usize> for TArray<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_mut(index).expect("TArray index out of bounds")
    }
}
impl<T> Drop for TArray<T> {
    fn drop(&mut self) {
        if !self.data.is_null() {
            unsafe {
                std::ptr::drop_in_place(std::ptr::slice_from_raw_parts_mut(
                    self.data,
                    self.num as usize,
                ));
                globals().gmalloc().free(self.data.cast());
            }
        }
    }
}
impl<T> Default for TArray<T> {
    fn default() -> Self {
        Self {
            data: std::ptr::null_mut(),
            num: 0,
            max: 0,
        }
    }
}

impl<T> From<&[T]> for TArray<T>
where
    T: Copy,
{
    fn from(value: &[T]) -> Self {
        if value.is_empty() {
            return Self::new();
        }
        unsafe { Self::from_slice_in(value, super::try_gmalloc().expect("GMalloc is not initialized")) }
    }
}
