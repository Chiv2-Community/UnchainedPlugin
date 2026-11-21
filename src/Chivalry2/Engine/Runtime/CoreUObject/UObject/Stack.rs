//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/CoreUObject/Public/UObject/Stack.h>.

#[cfg(feature = "kismet_log")]
use crate::Chivalry2::Engine::FName;
#[cfg(feature = "kismet_log")]
use crate::Chivalry2::Engine::UObject;
#[cfg(feature = "kismet_log")]
use crate::Chivalry2::Engine::TCHAR;
use crate::Chivalry2::Engine::*;
use std::ffi::c_void;
use std::mem::MaybeUninit;

unchained_macros::define_custom_resolver!(FFrameStep,
    delegate patternsleuth::resolvers::unreal::kismet::FFrameStep);
unchained_macros::define_custom_resolver!(FFrameStepExplicitProperty,
    delegate patternsleuth::resolvers::unreal::kismet::FFrameStepExplicitProperty);

#[cfg(feature = "kismet_log")]
define_pattern_resolver!(
    FFrame__KismetExecutionMessage,
    ["48 89 5C 24 08 57 48 83 EC 30 0F B6 DA 48 8B F9 80 FA 01 ?? ?? ?? ?? ?? ?? ?? ?? ?? BA",]
);

#[unchained_macros::runtime_bindings]
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
    /// # Safety
    /// Frame, context, and result storage must satisfy the current bytecode operation.
    #[unchained_macros::bind_resolved_func(
        self::FFrameStep
    )]
    pub unsafe fn Step(&mut self, context: *mut UObject, result: *mut c_void) {}

    /// Preserves the existing property-pointer ABI.
    ///
    /// # Safety
    /// Frame, property, and result storage must describe a valid compiled-in argument.
    #[unchained_macros::bind_resolved_func(
        self::FFrameStepExplicitProperty
    )]
    pub unsafe fn StepExplicitProperty(
        &mut self,
        result: *mut c_void,
        property: *const FProperty,
    ) {}

    pub unsafe fn arg<T: Sized>(self: &mut FFrame) -> T {
        let mut value: MaybeUninit<T> = MaybeUninit::zeroed();
        let ptr = value.as_mut_ptr() as *mut _;

        if self.code.is_null() {
            let cur = self.property_chain_for_compiled_in;
            self.property_chain_for_compiled_in = (*cur).next;
            self.StepExplicitProperty(ptr, cur as *const FProperty);
        } else {
            self.Step(self.object, ptr);
        }

        value.assume_init()
    }

    // Preserve the legacy return ABI pending native-signature verification.
    #[cfg(feature = "kismet_log")]
    #[unchained_macros::bind_resolved_func(FFrame__KismetExecutionMessage)]
    pub(crate) unsafe fn KismetExecutionMessage(
        text: *const TCHAR,
        kind: u8,
        name: FName,
    ) -> *mut UObject {
    }
}
