//! Bounded discovery of direct native callees in verified Unreal exec-thunk shapes.
//! This is not ABI inference. Indirect dispatch and tail calls fail closed.

use iced_x86::{Decoder, DecoderOptions, FlowControl, Instruction, Mnemonic, OpKind, Register};
use std::ops::Range;
use windows::Win32::{
    Foundation::HMODULE,
    System::{
        Diagnostics::Debug::RtlLookupFunctionEntry,
        Memory::{
            VirtualQuery, MEMORY_BASIC_INFORMATION, MEM_COMMIT, PAGE_EXECUTE_READ, PAGE_EXECUTE_READWRITE,
            PAGE_EXECUTE_WRITECOPY, PAGE_GUARD,
        },

        ProcessStatus::{GetModuleInformation, MODULEINFO},
        Threading::GetCurrentProcess,
    },
};

/// Parameter handling verified from the reflected UFunction metadata.
#[derive(Clone, Copy)]
pub enum ExecThunkParameters {
    /// At least one non-return parameter; require a recognized frame-step call.
    Stepped,
    /// No non-return parameters; require the supported inline Code advancement.
    NoArguments { frame_code_offset: usize },
}

fn registers(instruction: &Instruction, mnemonic: Mnemonic, left: Register, right: Register) -> bool {
    instruction.mnemonic() == mnemonic
        && instruction.op0_kind() == OpKind::Register
        && instruction.op0_register() == left
        && instruction.op1_kind() == OpKind::Register
        && instruction.op1_register() == right
}

fn frame_code(instruction: &Instruction, offset: usize) -> bool {
    instruction.memory_base() == Register::RDX
        && instruction.memory_index() == Register::None
        && instruction.memory_displacement64() == offset as u64
        && instruction.memory_size().size() == 8
}

fn no_argument_advance(instructions: &[Instruction], offset: usize) -> Result<u64, String> {
    for (index, window) in instructions.windows(7).enumerate() {
        let [load, zero, test, save, set, add, store] = window else {
            unreachable!()
        };

        let code = load.op0_register();
        let counter = zero.op0_register();
        let full_counter = counter.full_register();
        if load.mnemonic() != Mnemonic::Mov
            || load.op0_kind() != OpKind::Register
            || !code.is_gpr64()
            || code == Register::RDX
            || load.op1_kind() != OpKind::Memory
            || !frame_code(load, offset)
            || !counter.is_gpr32()
            || full_counter == code
            || full_counter == Register::RDX
            || !registers(zero, Mnemonic::Xor, counter, counter)
            || !registers(test, Mnemonic::Test, code, code)
            || save.mnemonic() != Mnemonic::Mov
            || save.op0_kind() != OpKind::Register
            || !save.op0_register().is_gpr64()
            || save.op1_kind() != OpKind::Register
            || save.op1_register() != Register::R8
            || [Register::RDX, code, full_counter].contains(&save.op0_register())
            || set.mnemonic() != Mnemonic::Setne
            || set.op0_kind() != OpKind::Register
            || !set.op0_register().is_gpr8()
            || set.op0_register().full_register() != full_counter
            || matches!(
                set.op0_register(),
                Register::AH | Register::BH | Register::CH | Register::DH
            )
            || !registers(add, Mnemonic::Add, full_counter, code)
            || store.mnemonic() != Mnemonic::Mov
            || store.op0_kind() != OpKind::Memory
            || !frame_code(store, offset)
            || store.op1_kind() != OpKind::Register
            || store.op1_register() != full_counter
        {
            continue;
        }

        // The prefix must not call helpers, branch around advancement, or replace the frame.
        if instructions[..index].iter().any(|instruction| {
            instruction.flow_control() != FlowControl::Next
                || (instruction.op0_kind() == OpKind::Register
                    && instruction.op0_register().full_register() == Register::RDX)
        }) {
            return Err("Unsupported prefix before no-argument frame advancement".into());
        }

        return Ok(store.next_ip());
    }

    Err("No supported no-argument frame advancement".into())
}

fn candidate(
    bytes: &[u8],
    thunk: usize,
    step: usize,
    explicit: usize,
    module: Range<usize>,
    parameters: ExecThunkParameters,
) -> Result<usize, String> {
    let instructions: Vec<_> = Decoder::with_ip(64, bytes, thunk as u64, DecoderOptions::NONE)
        .into_iter()
        .collect();
    let no_arguments = match parameters {
        ExecThunkParameters::Stepped => None,
        ExecThunkParameters::NoArguments { frame_code_offset } => {
            Some(no_argument_advance(&instructions, frame_code_offset)?)
        }
    };

    let mut target = None;
    let mut stepped = false;
    for instruction in instructions {
        if instruction.is_invalid() {
            return Err("Invalid instruction in exec thunk".into());
        }

        if instruction.mnemonic() == Mnemonic::Call {
            if instruction.op0_kind() != OpKind::NearBranch64 {
                return Err("Indirect calls in exec thunks require an explicit receiver resolver".into());
            }

            let callee = instruction.near_branch_target() as usize;
            if callee == step || callee == explicit {
                if no_arguments.is_some() {
                    return Err("Frame stepping contradicts no-argument metadata".into());
                }

                stepped = true;
                target = None;
            } else if (stepped || no_arguments.is_some_and(|ready| instruction.ip() >= ready)) && target.is_none() {
                if !module.contains(&callee) {
                    return Err("Exec callee is outside the game module".into());
                }

                target = Some(callee);
            }
        } else if matches!(
            instruction.flow_control(),
            FlowControl::UnconditionalBranch | FlowControl::ConditionalBranch
        ) {
            if no_arguments.is_some() && target.is_none() {
                return Err("Branch before no-argument native body".into());
            }

            let destination = instruction.near_branch_target() as usize;
            if !(thunk..thunk + bytes.len()).contains(&destination) {
                return Err("Exec thunk has an unsupported tail/external branch".into());
            }
        } else if instruction.flow_control() == FlowControl::IndirectBranch {
            return Err("Exec thunk has an indirect tail branch".into());
        }
    }

    target.ok_or_else(|| "No direct native body after parameter handling; inlined thunks are unsupported".into())
}

unsafe fn executable_range(address: usize, end: usize) -> bool {
    let mut info = MEMORY_BASIC_INFORMATION::default();
    let read = unsafe {
        VirtualQuery(
            Some(address as *const _),
            &mut info,
            size_of::<MEMORY_BASIC_INFORMATION>(),
        )
    };

    read != 0
        && info.State == MEM_COMMIT
        && !info.Protect.contains(PAGE_GUARD)
        && (info.Protect.0 & (PAGE_EXECUTE_READ.0 | PAGE_EXECUTE_READWRITE.0 | PAGE_EXECUTE_WRITECOPY.0)) != 0
        && (info.BaseAddress as usize)
            .checked_add(info.RegionSize)
            .is_some_and(|limit| end <= limit)
}

unsafe fn extent(address: usize, base: usize) -> Result<Range<usize>, String> {
    let mut image = 0;
    let entry = unsafe { RtlLookupFunctionEntry(address as u64, &mut image, None).as_ref() }
        .ok_or("Code has no x64 unwind entry; leaf/no-op bodies are unsupported")?;
    let start = image
        .checked_add(u64::from(entry.BeginAddress))
        .ok_or("Unwind address overflow")? as usize;
    let end = image
        .checked_add(u64::from(entry.EndAddress))
        .ok_or("Unwind address overflow")? as usize;
    if image as usize != base || start != address || end <= start || !unsafe { executable_range(start, end) } {
        return Err("Address is not an executable function entry in the game module".into());
    }

    Ok(start..end)
}

/// # Safety
/// `base` must identify the live game module; `step` and `explicit` must be its
/// verified frame-step entry addresses, not Rust wrappers or trampolines.
/// Only use with independently verified direct-call native declarations.
/// `parameters` must match reflection metadata; a return value is not an argument.
pub unsafe fn native_body(
    thunk: usize,
    base: usize,
    step: usize,
    explicit: usize,
    parameters: ExecThunkParameters,
) -> Result<usize, String> {
    let mut module = MODULEINFO::default();
    unsafe {
        GetModuleInformation(
            GetCurrentProcess(),
            HMODULE(base as isize),
            &mut module,
            size_of::<MODULEINFO>() as u32,
        )
    }
    .map_err(|error| error.to_string())?;
    let end = base
        .checked_add(module.SizeOfImage as usize)
        .ok_or("Module range overflow")?;
    let range = unsafe { extent(thunk, base) }?;
    if range.end > end {
        return Err("Exec thunk exceeds the game module".into());
    }

    let bytes = unsafe { std::slice::from_raw_parts(range.start as *const u8, range.len()) };
    let target = candidate(bytes, thunk, step, explicit, base..end, parameters)?;
    unsafe { extent(target, base) }?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::{candidate, ExecThunkParameters};

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
        assert_eq!(
            candidate(
                &bytes,
                0x1000,
                0x1100,
                0x1200,
                0x1000..0x2000,
                ExecThunkParameters::Stepped
            )
            .unwrap(),
            0x1300
        );
    }

    // Synthetic Code += (Code != nullptr), saving the VM result pointer in a nonvolatile register.
    const ADVANCE: &[u8] = &[
        0x48, 0x8b, 0x42, 0x48, // mov rax, [rdx+0x48]
        0x31, 0xc9, // xor ecx, ecx
        0x48, 0x85, 0xc0, // test rax, rax
        0x4c, 0x89, 0xc3, // mov rbx, r8
        0x0f, 0x95, 0xc1, // setne cl
        0x48, 0x01, 0xc1, // add rcx, rax
        0x48, 0x89, 0x4a, 0x48, // mov [rdx+0x48], rcx
    ];
    const ALTERNATE_ADVANCE: &[u8] = &[
        0x4c, 0x8b, 0x4a, 0x48, // mov r9, [rdx+0x48]
        0x31, 0xc0, // xor eax, eax
        0x4d, 0x85, 0xc9, // test r9, r9
        0x4c, 0x89, 0xc7, // mov rdi, r8
        0x0f, 0x95, 0xc0, // setne al
        0x4c, 0x01, 0xc8, // add rax, r9
        0x48, 0x89, 0x42, 0x48, // mov [rdx+0x48], rax
    ];

    fn no_arguments(bytes: &[u8]) -> Result<usize, String> {
        candidate(
            bytes,
            0x1000,
            0x1100,
            0x1200,
            0x1000..0x2000,
            ExecThunkParameters::NoArguments {
                frame_code_offset: 0x48,
            },
        )
    }

    #[test_case::test_case(ADVANCE; "static return storage")]
    #[test_case::test_case(ALTERNATE_ADVANCE; "instance return storage")]
    fn no_arguments_selects_body_before_conditional_cleanup(advance: &[u8]) {
        let mut bytes = advance.to_vec();
        call(&mut bytes, 0x1300);
        bytes.extend_from_slice(&[0x74, 0x05]); // Skip cleanup, not the native body.
        call(&mut bytes, 0x1400);
        bytes.push(0xc3);
        assert_eq!(no_arguments(&bytes).unwrap(), 0x1300);
        assert!(candidate(
            &bytes,
            0x1000,
            0x1100,
            0x1200,
            0x1000..0x2000,
            ExecThunkParameters::Stepped
        )
        .is_err());
        assert!(candidate(
            &bytes,
            0x1000,
            0x1100,
            0x1200,
            0x1000..0x2000,
            ExecThunkParameters::NoArguments {
                frame_code_offset: 0x40
            }
        )
        .is_err());
    }

    #[test]
    fn no_arguments_does_not_accept_an_arbitrary_first_call() {
        let mut bytes = Vec::new();
        call(&mut bytes, 0x1300);
        bytes.push(0xc3);
        assert!(no_arguments(&bytes).is_err());
        bytes = vec![0x48, 0x89, 0xca]; // Replace the frame register.
        bytes.extend_from_slice(ADVANCE);
        call(&mut bytes, 0x1300);
        bytes.push(0xc3);
        assert!(no_arguments(&bytes).is_err());
        bytes.clear();
        call(&mut bytes, 0x1400); // Constructor/helper before advancement.
        bytes.extend_from_slice(ADVANCE);
        call(&mut bytes, 0x1300);
        bytes.push(0xc3);
        assert!(no_arguments(&bytes).is_err());
    }

    #[test_case::test_case(0x1100; "unexpected frame step")]
    #[test_case::test_case(0x1200; "unexpected explicit step")]
    #[test_case::test_case(0x3000; "outside module")]
    fn no_arguments_rejects_invalid_callees(target: usize) {
        let mut bytes = ADVANCE.to_vec();
        call(&mut bytes, target);
        bytes.push(0xc3);
        assert!(no_arguments(&bytes).is_err());
    }

    #[test_case::test_case(&[0x74, 0]; "branch before body")]
    #[test_case::test_case(&[0xff, 0xd0]; "indirect call")]
    #[test_case::test_case(&[0xc3]; "inlined body")]
    #[test_case::test_case(&[0xe8, 0, 0]; "invalid instruction")]
    fn no_arguments_rejects_unsupported_bodies(body: &[u8]) {
        let mut bytes = ADVANCE.to_vec();
        bytes.extend_from_slice(body);
        assert!(no_arguments(&bytes).is_err());
    }

    #[test]
    fn no_arguments_rejects_changed_advancement_semantics() {
        for (offset, replacement) in [
            (5, 0xc0),  // Zero a different counter.
            (8, 0xc9),  // Test a different pointer.
            (14, 0xc5), // Set CH, not CL.
            (17, 0xc9), // Add the wrong pointer.
            (21, 0x40), // Store to another field.
        ] {
            let mut bytes = ADVANCE.to_vec();
            bytes[offset] = replacement;
            call(&mut bytes, 0x1300);
            bytes.push(0xc3);
            assert!(no_arguments(&bytes).is_err(), "accepted mutation at {offset}");
        }
    }

    #[test_case::test_case(&[0xe8, 0, 0]; "truncated call")]
    #[test_case::test_case(&[0xff, 0x50, 0x20]; "virtual call")]
    #[test_case::test_case(&[0xff, 0xe0]; "indirect tail")]
    #[test_case::test_case(&[0xe9, 0, 1, 0, 0]; "external tail")]
    #[test_case::test_case(&[0xc3]; "inlined or shared leaf")]
    fn unsupported_shapes_do_not_become_callable_addresses(bytes: &[u8]) {
        for parameters in [
            ExecThunkParameters::Stepped,
            ExecThunkParameters::NoArguments {
                frame_code_offset: 0x20,
            },
        ] {
            assert!(candidate(bytes, 0x1000, 0x1100, 0x1200, 0x1000..0x2000, parameters).is_err());
        }
    }
}
