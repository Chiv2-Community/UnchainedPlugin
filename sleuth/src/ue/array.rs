use std::alloc::Layout;
use std::ffi::c_void;

use crate::globals;

#[derive(Debug, Clone)]
#[repr(C)]
pub struct TArray<T> {
    data: *mut T,
    num: i32,
    max: i32,
}
impl<T> TArray<T> {
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
        let layout = Layout::array::<T>(capacity).expect("Layout error");
        Self {
            data: unsafe {
                globals()
                    .gmalloc()
                    .malloc(layout)
            } as *mut _,
            num: 0,
            max: capacity as i32,
        }
    }
    pub fn len(&self) -> usize {
        self.num as usize
    }
    pub fn capacity(&self) -> usize {
        self.max as usize
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn as_slice(&self) -> &[T] {
        if self.num == 0 {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(self.data, self.num as usize) }
        }
    }
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        if self.num == 0 {
            &mut []
        } else {
            unsafe { std::slice::from_raw_parts_mut(self.data as *mut _, self.num as usize) }
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
        let required_cap = self.len() + additional;
        if required_cap > self.capacity() {
            let new_cap = required_cap.next_power_of_two();
            let layout = Layout::array::<T>(new_cap).expect("Layout error");
            let new = unsafe {
                globals()
                    .gmalloc()
                    .realloc(self.data as *mut c_void, layout)
            } as *mut _;
            self.data = new;
            self.max = new_cap as i32;
        }
    }
    pub fn push(&mut self, new_value: T) {
        self.reserve(1);
        unsafe {
            std::ptr::write(self.data.add(self.num as usize), new_value);
            self.num += 1;
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
        let mut new = Self::with_capacity(value.len());
        new.extend_from_slice(value);
        new
    }
}
