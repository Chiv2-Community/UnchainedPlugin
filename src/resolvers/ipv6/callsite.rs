#[derive(Clone, Copy)]
pub struct CallPatch {
    pub address: usize,
    pub expected_target: usize,
    pub replacement: usize,
}

fn call_target(address: usize, bytes: [u8; 5]) -> Result<usize, String> {
    if bytes[0] != 0xe8 {
        return Err(format!("Expected an E8 call at {address:#x}"));
    }
    let displacement = i32::from_le_bytes(bytes[1..].try_into().unwrap());
    usize::try_from(address as i128 + 5 + displacement as i128)
        .map_err(|_| "Call target overflow".into())
}

fn relative_call(address: usize, target: usize) -> Result<[u8; 5], String> {
    let displacement = i32::try_from(target as i128 - address as i128 - 5)
        .map_err(|_| "Relay is outside rel32 range".to_string())?;
    let mut bytes = [0xe8, 0, 0, 0, 0];
    bytes[1..].copy_from_slice(&displacement.to_le_bytes());
    Ok(bytes)
}

fn relay(target: usize) -> [u8; 16] {
    // jmp qword ptr [rip]; the tail jump preserves the caller's ABI and stack arguments.
    let mut bytes = [0x90; 16];
    bytes[..6].copy_from_slice(&[0xff, 0x25, 0, 0, 0, 0]);
    bytes[6..14].copy_from_slice(&(target as u64).to_le_bytes());
    bytes
}

fn validate(calls: &[CallPatch], originals: &[[u8; 5]]) -> Result<(), String> {
    if calls.is_empty() || calls.len() != originals.len() || calls.len() > 4096 / 16 {
        return Err("Invalid call patch batch".into());
    }
    for (i, (call, bytes)) in calls.iter().zip(originals).enumerate() {
        if call_target(call.address, *bytes)? != call.expected_target {
            return Err(format!("Unexpected call target at {:#x}", call.address));
        }
        if call.address & 0xfff > 0xffb {
            return Err("Call crosses a page boundary".into());
        }
        if calls[..i]
            .iter()
            .any(|other| other.address.abs_diff(call.address) < 5)
        {
            return Err("Overlapping call patches".into());
        }
    }
    Ok(())
}

#[cfg(windows)]
pub use runtime::install;

#[cfg(windows)]
mod runtime {
    use super::*;
    use std::{ffi::c_void, mem::size_of, ptr};
    use windows::Win32::System::{
        Diagnostics::Debug::{FlushInstructionCache, ReadProcessMemory},
        Memory::{
            VirtualAlloc, VirtualFree, VirtualProtect, VirtualQuery, MEMORY_BASIC_INFORMATION,
            MEM_COMMIT, MEM_FREE, MEM_RELEASE, MEM_RESERVE, PAGE_EXECUTE_READ,
            PAGE_EXECUTE_READWRITE, PAGE_PROTECTION_FLAGS, PAGE_READWRITE,
        },
        Threading::GetCurrentProcess,
    };

    struct RelayPage(*mut c_void);

    impl Drop for RelayPage {
        fn drop(&mut self) {
            unsafe {
                let _ = VirtualFree(self.0, 0, MEM_RELEASE);
            }
        }
    }

    impl RelayPage {
        unsafe fn allocate(calls: &[CallPatch]) -> Result<Self, String> {
            const GRANULARITY: usize = 0x10000;
            let lower = calls
                .iter()
                .map(|call| (call.address + 5).saturating_sub(0x80000000))
                .max()
                .unwrap()
                .max(GRANULARITY);
            let upper = calls
                .iter()
                .map(|call| (call.address + 5).saturating_add(0x7fffffff))
                .min()
                .unwrap()
                .saturating_sub(4096);
            let mut cursor = lower;
            while cursor <= upper {
                let mut info = MEMORY_BASIC_INFORMATION::default();
                if VirtualQuery(
                    Some(cursor as *const c_void),
                    &mut info,
                    size_of::<MEMORY_BASIC_INFORMATION>(),
                ) == 0
                {
                    break;
                }
                let end = (info.BaseAddress as usize).saturating_add(info.RegionSize);
                if end <= cursor {
                    break;
                }
                if info.State == MEM_FREE {
                    let candidate = cursor.saturating_add(GRANULARITY - 1) & !(GRANULARITY - 1);
                    if candidate <= upper && candidate.saturating_add(4096) <= end {
                        let page = VirtualAlloc(
                            Some(candidate as *const c_void),
                            4096,
                            MEM_RESERVE | MEM_COMMIT,
                            PAGE_READWRITE,
                        );
                        if !page.is_null() {
                            return Ok(Self(page));
                        }
                        cursor = candidate.saturating_add(GRANULARITY);
                        continue;
                    }
                }
                cursor = end;
            }
            Err("No free relay page in range of all IPv6 call sites".into())
        }
    }

    // Restore in reverse order: two calls can share a page and thus nest protection changes.
    struct WritableCalls(Vec<(usize, PAGE_PROTECTION_FLAGS)>);

    impl Drop for WritableCalls {
        fn drop(&mut self) {
            while let Some((address, protection)) = self.0.pop() {
                let mut ignored = PAGE_PROTECTION_FLAGS(0);
                if unsafe { VirtualProtect(address as *const c_void, 5, protection, &mut ignored) }
                    .is_err()
                {
                    // Never continue with unexpectedly writable executable pages.
                    std::process::abort();
                }
            }
        }
    }

    /// Install only during preinit, before game threads can execute these call sites.
    /// All calls are validated and made writable before any instruction is changed.
    pub unsafe fn install(calls: &[CallPatch]) -> Result<(), String> {
        let mut originals = Vec::with_capacity(calls.len());
        for call in calls {
            let mut bytes = [0; 5];
            let mut read = 0;
            ReadProcessMemory(
                GetCurrentProcess(),
                call.address as *const c_void,
                bytes.as_mut_ptr().cast(),
                bytes.len(),
                Some(&mut read),
            )
            .map_err(|e| format!("Cannot read call at {:#x}: {e}", call.address))?;
            if read != bytes.len() {
                return Err("Partial call instruction read".into());
            }
            originals.push(bytes);
        }
        validate(calls, &originals)?;

        let page = RelayPage::allocate(calls)?;
        let mut replacements = Vec::with_capacity(calls.len());
        for (index, call) in calls.iter().enumerate() {
            let slot = page.0.cast::<u8>().add(index * 16);
            ptr::copy_nonoverlapping(relay(call.replacement).as_ptr(), slot, 16);
            replacements.push(relative_call(call.address, slot as usize)?);
        }
        let mut old = PAGE_PROTECTION_FLAGS(0);
        VirtualProtect(page.0, 4096, PAGE_EXECUTE_READ, &mut old)
            .map_err(|e| format!("Cannot protect IPv6 relays: {e}"))?;
        FlushInstructionCache(GetCurrentProcess(), Some(page.0), 4096)
            .map_err(|e| format!("Cannot flush IPv6 relays: {e}"))?;

        let mut writable = WritableCalls(Vec::new());
        for call in calls {
            let mut old = PAGE_PROTECTION_FLAGS(0);
            VirtualProtect(
                call.address as *const c_void,
                5,
                PAGE_EXECUTE_READWRITE,
                &mut old,
            )
            .map_err(|e| format!("Cannot protect call at {:#x}: {e}", call.address))?;
            writable.0.push((call.address, old));
        }
        for (call, bytes) in calls.iter().zip(&replacements) {
            ptr::copy_nonoverlapping(bytes.as_ptr(), call.address as *mut u8, 5);
        }
        if let Err(error) = FlushInstructionCache(GetCurrentProcess(), None, 0) {
            for (call, bytes) in calls.iter().zip(&originals) {
                ptr::copy_nonoverlapping(bytes.as_ptr(), call.address as *mut u8, 5);
            }
            if FlushInstructionCache(GetCurrentProcess(), None, 0).is_err() {
                std::process::abort();
            }
            return Err(format!(
                "Cannot flush IPv6 calls; restored originals: {error}"
            ));
        }
        drop(writable);
        // The patched calls need their relays for the lifetime of the process.
        std::mem::forget(page);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_forward_backward_and_boundary_calls() {
        let address = 0x140000000;
        for displacement in [i32::MIN, -1, 0, 1, i32::MAX] {
            let target = (address as i64 + 5 + displacement as i64) as usize;
            assert_eq!(
                call_target(address, relative_call(address, target).unwrap()).unwrap(),
                target
            );
        }
        assert!(relative_call(address, address + 5 + i32::MAX as usize + 1).is_err());
        assert!(relative_call(address, address + 5 - 0x80000001).is_err());
        assert!(call_target(address, [0x90; 5]).is_err());
    }

    #[test]
    fn relay_preserves_registers_and_accepts_distant_dll_targets() {
        let target = 0x7fff12345678;
        let bytes = relay(target);
        assert_eq!(&bytes[..6], &[0xff, 0x25, 0, 0, 0, 0]);
        assert_eq!(
            u64::from_le_bytes(bytes[6..14].try_into().unwrap()),
            target as u64
        );
    }

    #[test]
    fn rejects_the_whole_plan_if_any_call_is_wrong() {
        let calls: [CallPatch; 4] = std::array::from_fn(|index| CallPatch {
            address: 0x140010000 + index * 16,
            expected_target: 0x141000000,
            replacement: 0x7fff12345678,
        });
        let mut originals =
            calls.map(|call| relative_call(call.address, call.expected_target).unwrap());
        assert!(validate(&calls, &originals).is_ok());
        originals[3] = relative_call(calls[3].address, calls[3].expected_target + 1).unwrap();
        assert!(validate(&calls, &originals).is_err());
    }

    #[test]
    fn rejects_overlapping_and_cross_page_calls() {
        let call = CallPatch {
            address: 0x140010000,
            expected_target: 0x141000000,
            replacement: 0x7fff12345678,
        };
        let bytes = relative_call(call.address, call.expected_target).unwrap();
        assert!(validate(&[call, call], &[bytes, bytes]).is_err());
        let crossing = CallPatch {
            address: call.address + 0xffc,
            ..call
        };
        assert!(validate(
            &[crossing],
            &[relative_call(crossing.address, crossing.expected_target).unwrap()]
        )
        .is_err());
    }
}
