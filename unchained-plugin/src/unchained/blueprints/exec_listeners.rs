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
    unsafe { observation::observe_step(frame.cast(), result, 0, || FFrame::Step(frame, context, result)); }
}

#[unchained_macros_core::bind_func_detour(FFrame::StepExplicitProperty)]
unsafe fn observe_frame_step_explicit(
    frame: *mut FFrame,
    result: *mut c_void,
    property: *const FProperty,
) {
    unsafe {
        observation::observe_step(frame.cast(), result, property as usize,
            || FFrame::StepExplicitProperty(frame, result, property));
    }
}
