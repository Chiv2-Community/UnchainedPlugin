//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/GameFramework/OnlineReplStructs.h>.

use crate::chivalry2::Engine::*;

use std::os::raw::c_void;

#[repr(C)]
pub struct FUniqueNetIdRepl {
    pub(crate) vtable: *const usize,       // 0x0000
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
        if !object.IsValid() {
            return None;
        }
        Some(object.to_native_string())
    }
}
