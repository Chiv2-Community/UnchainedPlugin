//! Compile the actual host adapter against raw-receiver native declarations.
#![feature(arbitrary_self_types_pointers)]
#![allow(non_snake_case, dead_code)]

use std::ffi::c_void;

pub struct FFrame;

pub struct FProperty;

pub struct UObject;

#[unchained_macros_kismet::kismet_runtime_bindings("/Script/Fixture.Frame")]

impl FFrame {
    pub unsafe fn Step(self: *mut Self, context: *mut UObject, result: *mut c_void) {}

    pub unsafe fn StepExplicitProperty(self: *mut Self, result: *mut c_void, property: *const FProperty) {}
}

mod Chivalry2 {

    pub mod Engine {

        pub use crate::{FFrame, FProperty, UObject};
    }
}

#[path = "../../unchained-plugin/src/unchained/blueprints/exec_listeners.rs"]
mod actual_host_adapter;

#[test]

fn raw_receiver_signatures_match_without_mutability_overrides() {
    let _: &unchained_macros_core::NativeTarget<
        unsafe extern "C" fn(*mut FFrame, *mut UObject, *mut c_void),
        _,
        _,
        _,
        _,
    > = FFrame::__UNCHAINED_TARGET_Step;

    let _: &unchained_macros_core::NativeTarget<
        unsafe extern "C" fn(*mut FFrame, *mut c_void, *const FProperty),
        _,
        _,
        _,
        _,
    > = FFrame::__UNCHAINED_TARGET_StepExplicitProperty;
}
