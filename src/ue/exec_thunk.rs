//! Extracting the inner native a UFunction exec thunk calls, and detecting shipping-dummied natives.
//!
//! A native exec thunk (`execFoo`) steps its arguments off the `FFrame` and then `CALL`s the plain
//! C++ body (`UKismetSystemLibrary::PrintString`). In a debug build that call targets the real
//! function. In shipping the print-family bodies fold to a shared no-op stub. Native-body hooks
//! use this module to recover the C++ entry. Exec-thunk hooks are declared separately when the
//! wrapper itself is the target. This module also provides helpers for identifying the shared stub.
//!
//! The scan is bounded by the thunk's x64 unwind entry. It finds `FFrame::Step` and
//! `StepExplicitProperty` by their resolved addresses, then inspects the first call after the
//! last step. Indirect dispatch must be identified before any later cleanup calls. A direct
//! call encodes its target, while an indirect call may encode a byte offset into a receiver's
//! vtable. The hook binding supplies that receiver because the call operand alone does not
//! establish which object's or interface's vtable the thunk loaded.
//!
//! Call selection is a heuristic based on instruction order. The scan keeps the first eligible
//! call after the last recognized frame step in address order. An out-of-line argument block,
//! an inlined implementation, or a tail jump can hide the target or leave a cleanup call as the
//! candidate. The scan also needs Windows x64 unwind metadata to bound the thunk.
//!
//! A register-relative call only suggests a vtable byte offset. Verify the receiver and native
//! ABI before migrating a hook, ideally by comparing the recovered address with a known target
//! in the same build. The module address range is a coarse filter for candidate targets.

use std::sync::OnceLock;

use iced_x86::{Decoder, DecoderOptions, Mnemonic, OpKind, Register};
use windows::Win32::System::Diagnostics::Debug::RtlLookupFunctionEntry;

use crate::resolvers::asset_registry::o_GetDefaultObject;
use crate::resolvers::BASE_ADDR;
use crate::tools::hook_globals::globals;
use crate::ue::{try_read, UClass};

/// Generous upper bound on the module's executable range (image is ~100 MB).
const MODULE_SPAN: usize = 0x0800_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookTarget {
    /// The real inner native function (hook it directly as a normal `Called` detour).
    Inner(usize),
    /// A dummied/no-op callee: hook the exec thunk itself and step the frame.
    Exec(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InnerCall {
    Direct(usize),
    Indirect { candidate_slot: Option<usize> },
}

/// Best-effort: the inner native that `thunk` (a UFunction exec thunk) calls after stepping its
/// arguments. Returns `None` if the thunk's extent or inner call can't be located.
///
/// # Safety
/// `thunk` must be a live, readable code address, and the `FFrameStep` resolvers must be available.
pub unsafe fn inner_native_of(thunk: usize) -> Option<usize> {
    hook_native_of(thunk, || None)
}

/// Resolve a native call. Fetch the receiver only when the thunk yielded a candidate vtable slot.
///
/// # Safety
/// `thunk` must be a live exec thunk and the frame-step resolvers must be initialized. If the
/// callback supplies a receiver, it must be the kind of object or interface whose vtable the thunk
/// uses. The scanner identifies the slot but does not trace the receiver register's provenance.
pub(crate) unsafe fn hook_native_of(
    thunk: usize,
    receiver: impl FnOnce() -> Option<*const usize>,
) -> Option<usize> {
    match inner_call_of(thunk)? {
        InnerCall::Direct(target) => Some(target),
        InnerCall::Indirect { candidate_slot } => {
            let slot = candidate_slot?;
            native_vtable_slot(receiver()?, slot)
        }
    }
}

/// Supply a representative receiver for thunks that dispatch through their context UObject.
pub(crate) unsafe fn default_object_receiver(path: &str) -> Option<*const usize> {
    let class = crate::game::engine::get_uobject_from_path(path, false).cast::<UClass>();
    if class.is_null() {
        return None;
    }
    // Use the engine accessor to avoid relying on the UClass layout. `false` keeps hook
    // attachment from constructing a default object when the class has none yet.
    let object = CALL_ORIGINAL_SAFE!(GetDefaultObject(class, false)).ok()?;
    (!object.is_null()).then_some(object.cast())
}

fn native_vtable_slot(receiver: *const usize, slot: usize) -> Option<usize> {
    if slot % size_of::<usize>() != 0 {
        return None;
    }
    // The receiver's first pointer is its vtable. The decoded displacement is already in
    // bytes, so add it directly. Checked reads let an unavailable pointer fail resolution.
    let vtable = try_read(receiver).filter(|address| *address != 0)?;
    let target = try_read(vtable.checked_add(slot)? as *const usize)?;
    let base = *BASE_ADDR;
    (base..base + MODULE_SPAN)
        .contains(&target)
        .then_some(target)
}

/// Same safety requirements as [`inner_native_of`].
pub(crate) unsafe fn inner_call_of(thunk: usize) -> Option<InnerCall> {
    if thunk == 0 {
        return None;
    }
    let mut image_base = 0u64;
    let entry = RtlLookupFunctionEntry(thunk as u64, &mut image_base, None).as_ref()?;
    let start = image_base.checked_add(u64::from(entry.BeginAddress))?;
    let end = image_base.checked_add(u64::from(entry.EndAddress))?;
    if !(start..end).contains(&(thunk as u64)) {
        return None;
    }
    let bytes = std::slice::from_raw_parts(
        thunk as *const u8,
        (end - thunk as u64) as usize,
    );
    let step = globals().fframe_step() as usize;
    let step_x = globals().fframe_step_explicit_property() as usize;
    let lo = *BASE_ADDR;
    scan_inner_native(thunk, bytes, step, step_x, lo..lo + MODULE_SPAN)
}

fn scan_inner_native(
    thunk: usize,
    bytes: &[u8],
    step: usize,
    step_x: usize,
    module: std::ops::Range<usize>,
) -> Option<InnerCall> {
    let mut candidate = None;
    for instruction in Decoder::with_ip(64, bytes, thunk as u64, DecoderOptions::NONE) {
        if instruction.mnemonic() != Mnemonic::Call {
            continue;
        }
        if instruction.op0_kind() == OpKind::NearBranch64 {
            let target = instruction.near_branch_target() as usize;
            if target == step || target == step_x {
                candidate = None;
            } else if candidate.is_none() && module.contains(&target) {
                candidate = Some(InnerCall::Direct(target));
            }
        } else if candidate.is_none() {
            // A register-relative memory operand can be a vtable slot. The instruction alone
            // cannot establish which object supplied that register.
            let candidate_slot = (instruction.op0_kind() == OpKind::Memory
                && instruction.memory_index() == Register::None
                && !matches!(
                    instruction.memory_base(),
                    Register::None | Register::RIP | Register::EIP | Register::RSP | Register::RBP
                ))
            .then(|| instruction.memory_displacement64() as usize);
            candidate = Some(InnerCall::Indirect { candidate_slot });
        }
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::{scan_inner_native, InnerCall};

    #[test]
    fn bounded_scan_excludes_step_calls_from_neighboring_thunk() {
        const THUNK: usize = 0x1000;
        const STEP: usize = 0x1100;
        const STEP_EXPLICIT: usize = 0x1200;
        const NATIVE: usize = 0x1300;
        const CLEANUP: usize = 0x1400;

        let mut bytes = Vec::new();
        let emit_call = |bytes: &mut Vec<u8>, target: usize| {
            let next_instruction = THUNK + bytes.len() + 5;
            let displacement = i32::try_from(target as isize - next_instruction as isize).unwrap();
            bytes.push(0xE8);
            bytes.extend_from_slice(&displacement.to_le_bytes());
        };

        emit_call(&mut bytes, STEP);
        emit_call(&mut bytes, STEP_EXPLICIT);
        emit_call(&mut bytes, NATIVE);
        emit_call(&mut bytes, CLEANUP);
        bytes.push(0xC3);
        let thunk_end = bytes.len();
        emit_call(&mut bytes, STEP);

        let scan = |code| scan_inner_native(THUNK, code, STEP, STEP_EXPLICIT, 0x1000..0x2000);
        assert_eq!(scan(&bytes[..thunk_end]), Some(InnerCall::Direct(NATIVE)));
        assert_eq!(scan(&bytes), None);
    }

    #[test]
    fn scan_ignores_call_truncated_at_thunk_end() {
        let bytes = [0x90, 0xE8, 0x00, 0x01, 0x00];
        assert_eq!(
            scan_inner_native(0x1000, &bytes, 0x1100, 0x1200, 0x1000..0x2000),
            None
        );
    }

    #[test]
    fn scan_preserves_virtual_dispatch_ahead_of_direct_cleanup() {
        let bytes = [
            0xe8, 0xfb, 0x00, 0x00, 0x00, // call 0x1100 (frame step)
            0xff, 0x90, 0x08, 0x12, 0x00, 0x00, // call [rax + 0x1208]
            0xe8, 0xf0, 0x03, 0x00, 0x00, // call 0x1400 (cleanup)
            0xc3,
        ];
        assert_eq!(
            scan_inner_native(0x1000, &bytes, 0x1100, 0x1200, 0x1000..0x2000),
            Some(InnerCall::Indirect {
                candidate_slot: Some(0x1208)
            })
        );
    }
}

/// The shared no-op stub that shipping folds dummied native bodies into, derived once from a
/// known-dummied function (`UKismetSystemLibrary::PrintString`).
///
/// # Safety
/// Must be called after the UObject / StaticFindObject / FFrameStep resolvers are available.
pub unsafe fn shared_noop_stub() -> Option<usize> {
    static STUB: OnceLock<Option<usize>> = OnceLock::new();
    *STUB.get_or_init(|| unsafe {
        let print_string =
            crate::ue::find_native("/Script/Engine.KismetSystemLibrary", "PrintString")?;
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
