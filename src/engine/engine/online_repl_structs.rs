//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/GameFramework/OnlineReplStructs.h>.

#![allow(non_snake_case)]
use crate::engine::*;
use std::os::raw::c_void;

#[rust_ctables::cpp_struct]
pub struct FUniqueNetId {
    #[vtable(
        offset(0x28);
        is_valid(&self) -> bool;
        write_native_string(&self, output: *mut FString) -> *mut FString;
    )]
    vtable: *const (),
}

impl FUniqueNetId {
    /// # Safety
    /// This must be a live native ID with the supported vtable ABI.
    #[inline(always)]
    pub unsafe fn to_native_string(&self) -> String {
        let mut value = FString::new();
        self.write_native_string(&mut value);
        value.to_string()
    }
}

#[repr(C)]
pub struct FUniqueNetIdRepl {
    vtable: *const usize,                  // 0x0000
    pub id: *const FUniqueNetId,           // 0x0008 (shared pointer)
    pub reference_controller: *mut c_void, // 0x0010
    pub replication_bytes: TArray<u8>,     // 0x0018
} // Size: 0x28

impl FUniqueNetIdRepl {
    /// Return the native ID text, or None for a missing or invalid ID.
    ///
    /// # Safety
    /// The wrapper must contain a live native FUniqueNetId with the supported
    /// IsValid/ToString ABI. No ownership of the shared object is transferred.
    pub unsafe fn to_native_string(&self) -> Option<String> {
        let object = self.id.as_ref()?;
        if !object.is_valid() {
            return None;
        }
        Some(object.to_native_string())
    }
}

#[cfg(test)]
mod unique_net_id_tests {
    use super::*;

    fn missing_id() -> FUniqueNetIdRepl {
        FUniqueNetIdRepl {
            vtable: std::ptr::null(),
            id: std::ptr::null(),
            reference_controller: std::ptr::null_mut(),
            replication_bytes: TArray::new(),
        }
    }

    #[test]
    fn missing_id_has_no_text() {
        assert!(unsafe { missing_id().to_native_string() }.is_none());
    }

    #[test]
    fn invalid_id_does_not_call_to_string() {
        unsafe extern "C" fn invalid(_: *const FUniqueNetId) -> bool {
            false
        }
        unsafe extern "C" fn unexpected(_: *const FUniqueNetId, _: *mut FString) -> *mut FString {
            std::process::abort();
        }
        let vtable = FUniqueNetIdVTable {
            _pad_0: [0; 5],
            is_valid: invalid,
            write_native_string: unexpected,
        };
        let object = FUniqueNetId { vtable: &vtable };
        let mut id = missing_id();
        id.id = &object;
        unsafe {
            assert!(id.to_native_string().is_none());
        }
    }
}
