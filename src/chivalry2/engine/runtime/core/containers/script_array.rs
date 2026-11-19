//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/Containers/ScriptArray.h>.

use std::marker::PhantomData;

/// Heap-allocated native array storage. Borrow only; never frees its buffer.
#[repr(C)]
pub struct FScriptArray {
    data: *const u8,
    num: i32,
    max: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptArrayError {
    InvalidCount,
    NullData,
    InvalidStride,
    Overflow,
}

/// Borrowed runtime-stride elements, not an owned `TArray`.
pub struct FScriptArrayView<'a> {
    data: *const u8,
    len: usize,
    stride: usize,
    _borrow: PhantomData<&'a FScriptArray>,
}

impl FScriptArray {
    /// # Safety
    /// Storage must use the heap allocator. If validation succeeds, non-null
    /// data must address a live allocation of at least `max * stride` bytes,
    /// stable for the borrow, with no concurrent engine mutation.
    pub unsafe fn view(&self, stride: usize) -> Result<FScriptArrayView<'_>, ScriptArrayError> {
        if stride == 0 || stride > isize::MAX as usize {
            return Err(ScriptArrayError::InvalidStride);
        }
        if self.num < 0 || self.max < 0 || self.num > self.max {
            return Err(ScriptArrayError::InvalidCount);
        }
        if self.data.is_null() && self.max != 0 {
            return Err(ScriptArrayError::NullData);
        }
        let bytes = (self.max as usize)
            .checked_mul(stride)
            .filter(|bytes| *bytes <= isize::MAX as usize)
            .ok_or(ScriptArrayError::Overflow)?;
        (self.data as usize)
            .checked_add(bytes)
            .ok_or(ScriptArrayError::Overflow)?;
        Ok(FScriptArrayView {
            data: self.data,
            len: self.num as usize,
            stride,
            _borrow: PhantomData,
        })
    }
}

impl FScriptArrayView<'_> {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = *const u8> + '_ {
        // Construction checked the complete allocation extent and stride.
        (0..self.len).map(move |index| unsafe { self.data.add(index * self.stride) })
    }
}

const _: () = assert!(std::mem::size_of::<FScriptArray>() == 0x10);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borrowed_rows_use_runtime_stride_without_owning_storage() {
        let data = [11u32, 22, 33];
        let array = FScriptArray {
            data: data.as_ptr().cast(),
            num: 3,
            max: 3,
        };
        {
            let view = unsafe { array.view(4) }.unwrap();
            assert_eq!(view.len(), 3);
            let values: Vec<_> = view.iter().map(|p| unsafe { *p.cast::<u32>() }).collect();
            assert_eq!(values, [11, 22, 33]);
        }
        assert!(!std::mem::needs_drop::<FScriptArray>());
        assert!(!std::mem::needs_drop::<FScriptArrayView<'_>>());
        assert_eq!(data, [11, 22, 33]);
        assert_eq!(array.num, 3);
    }

    #[test]
    fn validates_counts_nulls_stride_and_extent() {
        let data = [0u8; 8];
        let mut array = FScriptArray {
            data: data.as_ptr(),
            num: 0,
            max: 0,
        };
        for (num, max) in [(-1, 0), (0, -1), (2, 1)] {
            array.num = num;
            array.max = max;
            assert_eq!(
                unsafe { array.view(1) }.err(),
                Some(ScriptArrayError::InvalidCount)
            );
        }
        array.num = 0;
        array.max = 0;
        array.data = std::ptr::null();
        assert_eq!(unsafe { array.view(4) }.unwrap().iter().count(), 0);
        array.max = 1;
        assert_eq!(
            unsafe { array.view(4) }.err(),
            Some(ScriptArrayError::NullData)
        );
        array.data = data.as_ptr();
        for stride in [0, usize::MAX] {
            assert_eq!(
                unsafe { array.view(stride) }.err(),
                Some(ScriptArrayError::InvalidStride)
            );
        }
        array.max = 2;
        assert_eq!(
            unsafe { array.view(isize::MAX as usize) }.err(),
            Some(ScriptArrayError::Overflow)
        );
        array.max = i32::MAX;
        assert_eq!(
            unsafe { array.view(isize::MAX as usize) }.err(),
            Some(ScriptArrayError::Overflow)
        );
        array.max = 1;
        array.data = usize::MAX as *const u8;
        assert_eq!(
            unsafe { array.view(1) }.err(),
            Some(ScriptArrayError::Overflow)
        );
    }
}
