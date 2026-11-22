//! Game-thread VM adapter. Physical Step hooks use core's shared address owner.

use std::ffi::c_void;
use crate::Chivalry2::Engine::{FFrame, FProperty, UObject};
use unchained_macros_kismet::observation;

// Raw receivers avoid imposing Rust-exclusive references on recursively used VM
// frames. These signatures exactly match the native binding declarations.
#[unchained_macros_core::bind_func_detour(FFrame::Step)]
unsafe fn observe_frame_step(
    frame: *mut FFrame,
    context: *mut UObject,
    result: *mut c_void,
) {
    let address = FFrame::__UNCHAINED_BINDING_Step.binding().address()
        .expect("Step trampoline must be published before enabling observation");
    let original: unsafe extern "C" fn(*mut FFrame, *mut UObject, *mut c_void) =
        unsafe { std::mem::transmute(address.get()) };
    unsafe { observation::observe_step(frame.cast(), result, 0, || original(frame, context, result)); }
}

#[unchained_macros_core::bind_func_detour(FFrame::StepExplicitProperty)]
unsafe fn observe_frame_step_explicit(
    frame: *mut FFrame,
    result: *mut c_void,
    property: *const FProperty,
) {
    let address = FFrame::__UNCHAINED_BINDING_StepExplicitProperty.binding().address()
        .expect("ExplicitProperty trampoline must be published before enabling observation");
    let original: unsafe extern "C" fn(*mut FFrame, *mut c_void, *const FProperty) =
        unsafe { std::mem::transmute(address.get()) };
    unsafe {
        observation::observe_step(frame.cast(), result, property as usize,
            || original(frame, result, property));
    }
}
