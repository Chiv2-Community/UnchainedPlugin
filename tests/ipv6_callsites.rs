// Also runnable on Linux without building the Windows-only plugin:
// rustc --edition 2021 --test tests/ipv6_callsites.rs -o /tmp/opencode/ipv6-tests
#[path = "../src/resolvers/ipv6/callsite.rs"]
mod callsite;
#[path = "../src/resolvers/ipv6/endpoint.rs"]
mod endpoint;

#[cfg(all(windows, test))]
mod windows_tests {
    use super::callsite::{install, CallPatch};
    use std::{ffi::c_void, mem::size_of, ptr};
    use windows::Win32::System::{
        Diagnostics::Debug::FlushInstructionCache,
        Memory::{
            VirtualAlloc, VirtualFree, VirtualProtect, VirtualQuery, MEMORY_BASIC_INFORMATION,
            MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_EXECUTE_READ, PAGE_PROTECTION_FLAGS,
            PAGE_READWRITE,
        },
        Threading::GetCurrentProcess,
    };

    struct CodePage(*mut c_void);

    impl Drop for CodePage {
        fn drop(&mut self) {
            unsafe {
                VirtualFree(self.0, 0, MEM_RELEASE).unwrap();
            }
        }
    }

    unsafe extern "C" fn sum(a: i32, b: i32, c: i32, d: i32, e: i32) -> i32 {
        a + b + c + d + e
    }

    #[test]
    fn redirects_only_validated_calls_and_preserves_the_windows_abi() {
        unsafe {
            let page = CodePage(VirtualAlloc(
                None,
                4096,
                MEM_RESERVE | MEM_COMMIT,
                PAGE_READWRITE,
            ));
            assert!(!page.0.is_null());
            let base = page.0 as usize;
            let original = base + 0x200;
            // Original returns its fifth (stack) argument.
            ptr::copy_nonoverlapping(
                [0x8b, 0x44, 0x24, 0x28, 0xc3].as_ptr(),
                original as *mut u8,
                5,
            );
            for index in 0..5 {
                let entry = base + index * 32;
                // Reserve shadow space and forward the fifth argument into the nested call.
                let prefix = [
                    0x48, 0x83, 0xec, 0x28, 0x8b, 0x44, 0x24, 0x50, 0x89, 0x44, 0x24, 0x20, 0xe8,
                ];
                ptr::copy_nonoverlapping(prefix.as_ptr(), entry as *mut u8, prefix.len());
                let displacement = (original as i64 - entry as i64 - 17) as i32;
                ptr::copy_nonoverlapping(
                    displacement.to_le_bytes().as_ptr(),
                    (entry + 13) as *mut u8,
                    4,
                );
                ptr::copy_nonoverlapping(
                    [0x48, 0x83, 0xc4, 0x28, 0xc3].as_ptr(),
                    (entry + 17) as *mut u8,
                    5,
                );
            }
            let mut old = PAGE_PROTECTION_FLAGS(0);
            VirtualProtect(page.0, 4096, PAGE_EXECUTE_READ, &mut old).unwrap();
            FlushInstructionCache(GetCurrentProcess(), Some(page.0), 4096).unwrap();

            type Caller = unsafe extern "C" fn(i32, i32, i32, i32, i32) -> i32;
            let callers: [Caller; 5] =
                std::array::from_fn(|index| std::mem::transmute(base + index * 32));
            let calls: [CallPatch; 4] = std::array::from_fn(|index| CallPatch {
                address: base + index * 32 + 12,
                expected_target: original,
                replacement: sum as *const () as usize,
            });
            let mut bad = calls;
            bad[3].expected_target += 1;
            assert!(install(&bad).is_err());
            for caller in callers {
                assert_eq!(caller(1, 2, 3, 4, 5), 5);
            }

            install(&calls).unwrap();
            for caller in &callers[..4] {
                assert_eq!(caller(1, 2, 3, 4, 5), 15);
            }
            // A fifth, unrelated call to the same original function stays untouched.
            assert_eq!(callers[4](1, 2, 3, 4, 5), 5);
            let mut info = MEMORY_BASIC_INFORMATION::default();
            assert_ne!(
                VirtualQuery(
                    Some(page.0),
                    &mut info,
                    size_of::<MEMORY_BASIC_INFORMATION>()
                ),
                0
            );
            assert_eq!(info.Protect, PAGE_EXECUTE_READ);
        }
    }
}
