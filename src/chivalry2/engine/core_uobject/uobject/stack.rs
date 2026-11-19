//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/UObject/Stack.h>.

#[cfg(feature = "kismet_log")]
use crate::chivalry2::engine::FName;
#[cfg(feature = "kismet_log")]
use crate::chivalry2::engine::UObject;
#[cfg(feature = "kismet_log")]
use crate::chivalry2::engine::TCHAR;
use crate::chivalry2::engine::*;
use crate::globals;
use std::ffi::c_void;
use std::mem::MaybeUninit;

#[cfg(feature = "kismet_log")]
define_pattern_resolver!(
    KismetExecutionMessage,
    ["48 89 5C 24 08 57 48 83 EC 30 0F B6 DA 48 8B F9 80 FA 01 ?? ?? ?? ?? ?? ?? ?? ?? ?? BA",]
);

#[derive(Debug)]
#[repr(C)]
pub struct FFrame {
    pub base: FOutputDevice,
    pub node: *const c_void,
    pub object: *mut UObject,
    pub code: *const c_void,
    pub locals: *const c_void,
    pub most_recent_property: *const FProperty,
    pub most_recent_property_address: *const c_void,
    pub flow_stack: [u8; 0x30],
    pub previous_frame: *const FFrame,
    pub out_parms: *const c_void,
    pub property_chain_for_compiled_in: *const FField,
    pub current_native_function: *const c_void,
    pub b_array_context_failed: bool,
}

#[unchained_macros::runtime_bindings]
impl FFrame {
    pub unsafe fn arg<T: Sized>(self: &mut FFrame) -> T {
        let mut value: MaybeUninit<T> = MaybeUninit::zeroed();
        let ptr = value.as_mut_ptr() as *mut _;

        if self.code.is_null() {
            let cur = self.property_chain_for_compiled_in;
            self.property_chain_for_compiled_in = (*cur).next;
            globals().fframe_step_explicit_property()(self, ptr, cur as *const FProperty);
        } else {
            globals().fframe_step()(self, self.object, ptr);
        }

        value.assume_init()
    }

    // Preserve the legacy return ABI pending native-signature verification.
    #[cfg(feature = "kismet_log")]
    #[unchained_macros::bind_resolved_func(KismetExecutionMessage)]
    pub(crate) unsafe fn kismet_execution_message(
        text: *const TCHAR,
        kind: u8,
        name: FName,
    ) -> *mut UObject {
    }
}

pub type FnFFrameStep = unsafe extern "system" fn(
    stack: &mut crate::chivalry2::engine::core_uobject::uobject::stack::FFrame,
    *mut UObject,
    result: *mut c_void,
);

pub type FnFFrameStepExplicitProperty = unsafe extern "system" fn(
    stack: &mut crate::chivalry2::engine::core_uobject::uobject::stack::FFrame,
    result: *mut c_void,
    property: *const FProperty,
);
