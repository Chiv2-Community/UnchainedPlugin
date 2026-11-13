use crate::{globals, ue::FString};
use super::UEHash;

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u32)]
pub enum EFindName {
    Find,
    Add,
    ReplaceNotSafeForThreading,
}

#[derive(Default, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
#[repr(C)]
pub struct FName {
    pub comparison_index: FNameEntryId,
    pub number: u32,
}
/// Entry address of a game function from the image base and its resolved offset.
fn entry_address(base: usize, offset: Option<u64>, name: &str) -> Result<usize, String> {
    match offset {
        Some(offset) if offset != 0 => Ok(base + offset as usize),
        _ => Err(format!("No address resolved for {name}")),
    }
}

/// Calls the game's `FName::FName(const TCHAR*, EFindName)` at its entry address.
///
/// This deliberately does not go through the `FNameCtorWchar` detour's
/// trampoline: that trampoline is a copy of the function's first bytes taken
/// when the plugin attached, and it breaks (access violation at entry +5) when
/// another tool patches the function entry afterwards, as UE4SS does for its
/// FName constructor check. A call to the entry itself runs through whatever
/// is installed there.
///
/// # Safety
/// `text` must point to a NUL-terminated UTF-16 string; the engine's name
/// table must be initialized.
pub unsafe fn fname_ctor_wchar(name: &mut FName, text: *const super::TCHAR, find_type: EFindName) -> Result<(), String> {
    type Ctor = unsafe extern "C" fn(*mut FName, *const super::TCHAR, EFindName) -> *mut FName;
    static ENTRY: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    let entry = match ENTRY.get() {
        Some(entry) => *entry,
        None => {
            let entry = entry_address(
                *crate::resolvers::BASE_ADDR,
                crate::current_build_offset("FNameCtorWchar"),
                "FNameCtorWchar",
            )?;
            *ENTRY.get_or_init(|| entry)
        }
    };
    let ctor: Ctor = std::mem::transmute(entry);
    ctor(name, text, find_type);
    Ok(())
}

impl FName {

    pub unsafe fn new(string: &FString) -> FName {
        let mut ret = FName::default();
        fname_ctor_wchar(&mut ret, string.as_ptr(), EFindName::Add)
            .expect("Failed to construct FName (Add)");
        ret
    }
    pub unsafe fn find(string: &FString) -> FName {
        let mut ret = FName::default();
        fname_ctor_wchar(&mut ret, string.as_ptr(), EFindName::Find)
            .expect("Failed to construct FName (Find)");
        ret
    }
}

#[cfg(test)]
mod entry_tests {
    use super::entry_address;

    #[test]
    fn entry_is_base_plus_offset_and_missing_offset_is_an_error() {
        assert_eq!(entry_address(0x7ff7_1e9b_0000, Some(0x1e0_4650), "F"), Ok(0x7ff7_207b_4650));
        assert!(entry_address(0x1000, None, "F").is_err());
        assert!(entry_address(0x1000, Some(0), "F").is_err());
    }
}
impl std::fmt::Debug for FName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FName({self})")
    }
}

#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
#[repr(C)]
pub struct FNameEntryId {
    pub value: u32,
}

impl std::fmt::Display for FName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut string = FString::new();
        unsafe {
            globals().fname_to_string()(self, &mut string);
        };
        write!(f, "{string}")
    }
}
impl UEHash for FNameEntryId {
    fn ue_hash(&self) -> u32 {
        let value = self.value;
        (value >> 4) + value.wrapping_mul(0x10001) + (value >> 0x10).wrapping_mul(0x80001)
    }
}

impl UEHash for FName {
    fn ue_hash(&self) -> u32 {
        self.comparison_index.ue_hash() + self.number
    }
}
