//! Extracting the inner native a UFunction exec thunk calls, and detecting shipping-dummied natives.
//!
//! A native exec thunk (`execFoo`) steps its arguments off the `FFrame` and then `CALL`s the plain
//! C++ body (`UKismetSystemLibrary::PrintString`). In a debug build that call targets the real
//! function; in shipping the print/warning/text bodies are all folded to one shared no-op stub, so
//! the inner callee of a dummied thunk is that stub. This module recovers the inner callee and uses
//! it to decide whether to hook the inner function directly (as `Called`) or fall back to hooking
//! the exec thunk and stepping the frame.
//!
//! The scan is best-effort. It locates the `FFrame::Step` / `StepExplicitProperty` calls by their
//! known resolved addresses (so those are unambiguous), then takes the first `CALL rel32` after the
//! last step whose target lands inside the game module — that is the `P_NATIVE_BEGIN` inner call,
//! ahead of the `FMemory::Free` cleanup. The no-op stub address is derived at runtime from a
//! known-dummied function (`PrintString`), so nothing here is hard-coded to a build.

use std::sync::OnceLock;

use crate::resolvers::BASE_ADDR;
use crate::tools::hook_globals::globals;

/// How far into the thunk to scan. Thunks are small (execPrintString is 475 bytes); the inner call
/// always appears well before this. Reading a little past the function stays within `.text`.
const SCAN_LEN: usize = 0x400;
/// Generous upper bound on the module's executable range (image is ~100 MB).
const MODULE_SPAN: usize = 0x0800_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookTarget {
    /// The real inner native function (hook it directly as a normal `Called` detour).
    Inner(usize),
    /// A dummied/no-op callee: hook the exec thunk itself and step the frame.
    Exec(usize),
}

#[inline]
unsafe fn call_target(thunk: usize, bytes: &[u8], pos: usize) -> Option<usize> {
    if bytes.get(pos).copied()? != 0xE8 || pos + 5 > bytes.len() {
        return None;
    }
    let rel = i32::from_le_bytes([bytes[pos + 1], bytes[pos + 2], bytes[pos + 3], bytes[pos + 4]]);
    let after = (thunk + pos + 5) as i64;
    Some((after + rel as i64) as usize)
}

/// Best-effort: the inner native that `thunk` (a UFunction exec thunk) calls after stepping its
/// arguments. Returns `None` if the step calls / inner call can't be located.
///
/// # Safety
/// `thunk` must be a live, readable code address, and the `FFrameStep` resolvers must be available.
pub unsafe fn inner_native_of(thunk: usize) -> Option<usize> {
    if thunk == 0 {
        return None;
    }
    let step = globals().fframe_step() as usize;
    let step_x = globals().fframe_step_explicit_property() as usize;
    let bytes = std::slice::from_raw_parts(thunk as *const u8, SCAN_LEN);

    // End of the last argument-step call (StepExplicitProperty on the compiled-in path).
    let mut last_step_end = 0usize;
    let mut i = 0usize;
    while i + 5 <= bytes.len() {
        if let Some(t) = call_target(thunk, bytes, i) {
            if t == step || t == step_x {
                last_step_end = i + 5;
            }
        }
        i += 1;
    }

    // First in-module call after the last step = the P_NATIVE_BEGIN inner call.
    let lo = *BASE_ADDR;
    let hi = lo + MODULE_SPAN;
    let mut j = last_step_end;
    while j + 5 <= bytes.len() {
        if let Some(t) = call_target(thunk, bytes, j) {
            if t != step && t != step_x && t >= lo && t < hi {
                return Some(t);
            }
        }
        j += 1;
    }
    None
}

/// The shared no-op stub that shipping folds dummied native bodies into, derived once from a
/// known-dummied function (`UKismetSystemLibrary::PrintString`).
///
/// # Safety
/// Must be called after the UObject / StaticFindObject / FFrameStep resolvers are available.
pub unsafe fn shared_noop_stub() -> Option<usize> {
    static STUB: OnceLock<Option<usize>> = OnceLock::new();
    *STUB.get_or_init(|| unsafe {
        let print_string = crate::ue::find_native("/Script/Engine.KismetSystemLibrary", "PrintString")?;
        inner_native_of(print_string)
    })
}

/// Whether `thunk`'s inner callee is the shared no-op stub (i.e. the function is dummied in this
/// shipping build).
///
/// # Safety
/// Same preconditions as [`inner_native_of`].
pub unsafe fn is_dummied(thunk: usize) -> bool {
    match (inner_native_of(thunk), shared_noop_stub()) {
        (Some(inner), Some(stub)) => inner == stub,
        _ => false,
    }
}

/// Decide how to hook a native by `(class_path, name)`: hook the real inner function directly when
/// it exists (debug builds), else hook the exec thunk and step the frame (dummied shipping bodies).
///
/// # Safety
/// Same preconditions as [`inner_native_of`]; call during/after postinit.
pub unsafe fn resolve_hook_target(class_path: &str, name: &str) -> Option<HookTarget> {
    let thunk = crate::ue::find_native(class_path, name)?;
    if is_dummied(thunk) {
        return Some(HookTarget::Exec(thunk));
    }
    match inner_native_of(thunk) {
        Some(inner) => Some(HookTarget::Inner(inner)),
        None => Some(HookTarget::Exec(thunk)),
    }
}
