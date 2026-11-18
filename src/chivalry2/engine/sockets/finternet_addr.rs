//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Sockets/Public/IPAddress.h>.

#![allow(non_snake_case)]
use crate::engine::*;

/// Partial socket address vtable. ToString is slot 15 in the supported builds.
#[rust_ctables::cpp_struct]
pub struct FInternetAddr {
    #[vtable(
        offset(0x78);
        write_native_string(&self, output: *mut FString, append_port: bool) -> *mut FString;
    )]
    vtable: *const (),
}

impl FInternetAddr {
    /// # Safety
    /// This must be a live native socket address with the supported vtable ABI.
    #[inline(always)]
    pub unsafe fn to_native_string(&self, append_port: bool) -> String {
        let mut output = FString::new();
        self.write_native_string(&mut output, append_port);
        output.to_string()
    }
}
