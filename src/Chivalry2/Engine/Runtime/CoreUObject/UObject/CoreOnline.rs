//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/UObject/CoreOnline.h>.

use crate::Chivalry2::Engine::*;

#[unchained_macros::runtime_bindings]
#[rust_ctables::cpp_struct]
pub struct FUniqueNetId {
    #[vtable(
        offset(0x28);
        IsValid(&self) -> bool;
        ToString(&self, output: *mut FString) -> *mut FString;
    )]
    vtable: *const (),
}

impl FUniqueNetId {
    /// # Safety
    /// This must be a live native ID with the supported vtable ABI.
    #[inline(always)]
    pub unsafe fn to_native_string(&self) -> String {
        let mut value = FString::new();
        self.ToString(&mut value);
        value.to_string()
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
            IsValid: invalid,
            ToString: unexpected,
        };
        let object = FUniqueNetId { vtable: &vtable };
        let mut id = missing_id();
        id.id = &object;
        unsafe {
            assert!(id.to_native_string().is_none());
        }
    }
}
