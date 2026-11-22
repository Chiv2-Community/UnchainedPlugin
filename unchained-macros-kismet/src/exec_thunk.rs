//! Bounded discovery of direct native callees in verified Unreal exec-thunk shapes.
//! This is not ABI inference. Indirect dispatch and tail calls fail closed.

use iced_x86::{Decoder, DecoderOptions, FlowControl, Mnemonic, OpKind};
use std::ops::Range;
#[cfg(windows)]
use windows::Win32::{
    Foundation::HMODULE,
    System::{
        Diagnostics::Debug::RtlLookupFunctionEntry,
        Memory::{VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT, PAGE_EXECUTE_READ,
            PAGE_EXECUTE_READWRITE, PAGE_EXECUTE_WRITECOPY, PAGE_GUARD},
        ProcessStatus::{GetModuleInformation, MODULEINFO},
        Threading::GetCurrentProcess,
    },
};

fn candidate(bytes: &[u8], thunk: usize, step: usize, explicit: usize, module: Range<usize>) -> Result<usize, String> {
    let mut target = None;
    let mut stepped = false;
    for instruction in Decoder::with_ip(64, bytes, thunk as u64, DecoderOptions::NONE) {
        if instruction.is_invalid() { return Err("Invalid instruction in exec thunk".into()); }
        if instruction.mnemonic() == Mnemonic::Call {
            if instruction.op0_kind() != OpKind::NearBranch64 {
                return Err("Indirect calls in exec thunks require an explicit receiver resolver".into());
            }
            let callee = instruction.near_branch_target() as usize;
            if callee == step || callee == explicit {
                stepped = true;
                target = None;
            } else if stepped && target.is_none() {
                if !module.contains(&callee) { return Err("Exec callee is outside the game module".into()); }
                target = Some(callee);
            }
        } else if matches!(instruction.flow_control(), FlowControl::UnconditionalBranch | FlowControl::ConditionalBranch) {
            let destination = instruction.near_branch_target() as usize;
            if !(thunk..thunk + bytes.len()).contains(&destination) {
                return Err("Exec thunk has an unsupported tail/external branch".into());
            }
        } else if instruction.flow_control() == FlowControl::IndirectBranch {
            return Err("Exec thunk has an indirect tail branch".into());
        }
    }
    target.ok_or_else(|| "No direct native body after frame stepping; inlined/no-argument thunks are unsupported".into())
}

#[cfg(windows)]
unsafe fn executable_range(address: usize, end: usize) -> bool {
    let mut info = MEMORY_BASIC_INFORMATION::default();
    let read = unsafe { VirtualQuery(Some(address as *const _), &mut info, size_of::<MEMORY_BASIC_INFORMATION>()) };
    read != 0 && info.State == MEM_COMMIT && !info.Protect.contains(PAGE_GUARD)
        && (info.Protect.0 & (PAGE_EXECUTE_READ.0 | PAGE_EXECUTE_READWRITE.0 | PAGE_EXECUTE_WRITECOPY.0)) != 0
        && (info.BaseAddress as usize).checked_add(info.RegionSize).is_some_and(|limit| end <= limit)
}

#[cfg(windows)]
unsafe fn extent(address: usize, base: usize) -> Result<Range<usize>, String> {
    let mut image = 0;
    let entry = unsafe { RtlLookupFunctionEntry(address as u64, &mut image, None).as_ref() }
        .ok_or("Code has no x64 unwind entry; leaf/no-op bodies are unsupported")?;
    let start = image.checked_add(u64::from(entry.BeginAddress)).ok_or("Unwind address overflow")? as usize;
    let end = image.checked_add(u64::from(entry.EndAddress)).ok_or("Unwind address overflow")? as usize;
    if image as usize != base || start != address || end <= start || !unsafe { executable_range(start, end) } {
        return Err("Address is not an executable function entry in the game module".into());
    }
    Ok(start..end)
}

/// # Safety
/// `base` must identify the live game module; `step` and `explicit` must be its
/// verified frame-step entry addresses, not Rust wrappers or trampolines.
/// Only use with independently verified direct-call native declarations.
#[cfg(windows)]
pub unsafe fn native_body(thunk: usize, base: usize, step: usize, explicit: usize) -> Result<usize, String> {
    let mut module = MODULEINFO::default();
    unsafe { GetModuleInformation(GetCurrentProcess(), HMODULE(base as isize), &mut module, size_of::<MODULEINFO>() as u32) }
        .map_err(|error| error.to_string())?;
    let end = base.checked_add(module.SizeOfImage as usize).ok_or("Module range overflow")?;
    let range = unsafe { extent(thunk, base) }?;
    if range.end > end { return Err("Exec thunk exceeds the game module".into()); }
    let bytes = unsafe { std::slice::from_raw_parts(range.start as *const u8, range.len()) };
    let target = candidate(bytes, thunk, step, explicit, base..end)?;
    unsafe { extent(target, base) }?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::candidate;

    fn call(bytes: &mut Vec<u8>, target: usize) {
        let next = 0x1000 + bytes.len() + 5;
        bytes.push(0xe8);
        bytes.extend_from_slice(&i32::try_from(target as isize - next as isize).unwrap().to_le_bytes());
    }

    #[test]
    fn direct_body_precedes_cleanup_after_the_final_step() {
        let mut bytes = Vec::new();
        call(&mut bytes, 0x1100);
        call(&mut bytes, 0x1200);
        call(&mut bytes, 0x1300);
        call(&mut bytes, 0x1400);
        bytes.push(0xc3);
        assert_eq!(candidate(&bytes, 0x1000, 0x1100, 0x1200, 0x1000..0x2000).unwrap(), 0x1300);
    }

    #[test_case::test_case(&[0xe8, 0, 0]; "truncated call")]
    #[test_case::test_case(&[0xff, 0x50, 0x20]; "virtual call")]
    #[test_case::test_case(&[0xff, 0xe0]; "indirect tail")]
    #[test_case::test_case(&[0xe9, 0, 1, 0, 0]; "external tail")]
    #[test_case::test_case(&[0xc3]; "inlined or shared leaf")]
    fn unsupported_shapes_do_not_become_callable_addresses(bytes: &[u8]) {
        assert!(candidate(bytes, 0x1000, 0x1100, 0x1200, 0x1000..0x2000).is_err());
    }
}
