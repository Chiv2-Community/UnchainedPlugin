//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Sockets/Public/IPAddress.h>.

use crate::chivalry2::engine::*;

/// Partial socket address vtable. ToString is slot 15 in the supported builds.
#[rust_ctables::cpp_struct]
pub struct FInternetAddr {
    #[vtable(
        offset(0x78);
        ToString(&self, output: *mut FString, append_port: bool) -> *mut FString;
    )]
    vtable: *const (),
}

impl FInternetAddr {
    /// # Safety
    /// This must be a live native socket address with the supported vtable ABI.
    #[inline(always)]
    pub unsafe fn to_native_string(&self, append_port: bool) -> String {
        let mut output = FString::new();
        self.ToString(&mut output, append_port);
        output.to_string()
    }
}
