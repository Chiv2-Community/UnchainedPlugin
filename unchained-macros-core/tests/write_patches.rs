use std::mem::MaybeUninit;
use unchained_macros_core::{Patch, PatchWriter, WriteValue};

#[derive(Default)]
struct Writer(usize);
impl PatchWriter for Writer {
    unsafe fn bytes(&mut self, address: u64, bytes: &[u8]) -> Result<(), String> {
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), address as *mut u8, bytes.len());
        }
        self.0 += 1;
        Ok(())
    }
    unsafe fn nop(&mut self, address: u64, count: usize) -> Result<(), String> {
        unsafe {
            std::ptr::write_bytes(address as *mut u8, 0x90, count);
        }
        self.0 += 1;
        Ok(())
    }
    unsafe fn write(&mut self, address: u64, value: &WriteValue) -> Result<(), String> {
        unsafe {
            std::ptr::copy_nonoverlapping(
                value.as_ptr().cast::<MaybeUninit<u8>>(),
                address as *mut MaybeUninit<u8>,
                value.size(),
            );
        }
        self.0 += 1;
        Ok(())
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Padded {
    tag: u8,
    value: u32,
}
static PADDED: Padded = Padded {
    tag: 7,
    value: 0x12345678,
};
static STATIC_WRITE: Patch = Patch::write_ref(0, &PADDED);

#[test_case::test_case(false; "owned representation")]
#[test_case::test_case(true; "borrowed static representation")]
fn typed_writes_preserve_fields_without_reading_uninitialized_padding(borrowed: bool) {
    let patch = if borrowed {
        STATIC_WRITE.clone()
    } else {
        Patch::write(0, PADDED).clone()
    };
    let mut target = MaybeUninit::<Padded>::uninit();
    let mut writer = Writer::default();
    unsafe {
        patch.apply(target.as_mut_ptr() as u64, &mut writer).unwrap();
    }
    let target = unsafe { target.assume_init() };
    assert_eq!(target.tag, PADDED.tag);
    assert_eq!(target.value, PADDED.value);
    assert_eq!(writer.0, 1);
}

#[repr(C)]
#[derive(Clone, Copy)]
struct PointerValue {
    tag: u8,
    value: *const u64,
}

#[test]
fn owned_writes_preserve_raw_pointer_values_and_padding() {
    static VALUE: u64 = 42;
    let patch = Patch::write(
        0,
        PointerValue {
            tag: 9,
            value: &raw const VALUE,
        },
    )
    .clone();
    let mut target = MaybeUninit::<PointerValue>::uninit();
    unsafe {
        patch.apply(target.as_mut_ptr() as u64, &mut Writer::default()).unwrap();
    }
    let target = unsafe { target.assume_init() };
    assert_eq!(target.tag, 9);
    assert_eq!(target.value, &raw const VALUE);
    assert_eq!(unsafe { *target.value }, 42);
}

#[test]
fn unaligned_values_fail_before_the_writer() {
    let patch = Patch::write(0, 7u32);
    let mut writer = Writer::default();
    assert!(unsafe { patch.apply(1, &mut writer) }
        .unwrap_err()
        .contains("Unaligned"));
    assert_eq!(writer.0, 0);
}
