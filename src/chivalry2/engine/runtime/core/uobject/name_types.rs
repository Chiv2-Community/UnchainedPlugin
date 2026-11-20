//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/UObject/NameTypes.h>.

use crate::chivalry2::engine::TCHAR;
use crate::globals;

use crate::chivalry2::engine::FString;
use crate::chivalry2::engine::UEHash;

use unchained_macros::bind_resolved_func;

use widestring::U16CString;

define_pattern_resolver!(
    FNameCtorWchar,
    Simple,
    ["48 89 5C 24 ?? 57 48 83 EC 30 48 8B D9 48 89 54 24 ??"]
);

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

impl FName {
    /// Call the constructor through the entry, never a
    /// copied trampoline: UE4SS may patch this entry after plugin initialization.
    ///
    /// # Safety
    /// Self must be writable native FName storage, text NUL-terminated UTF-16,
    /// and the engine name table initialized.
    #[bind_resolved_func(self::FNameCtorWchar)]
    pub unsafe fn FName(
        self: *mut Self,
        text: *const crate::chivalry2::engine::TCHAR,
        find_type: EFindName,
    ) -> *mut Self {
    }

    pub fn with_type(path: &str, find_type: EFindName) -> Self {
        let wide_path = U16CString::from_str(path).unwrap();
        let mut name = Self::default();
        unsafe {
            (&raw mut name).FName(wide_path.as_ptr(), find_type);
        }
        if name.comparison_index.value == 0 {
            unsafe {
                (&raw mut name).FName(wide_path.as_ptr(), EFindName::Add);
            }
        }
        if name.comparison_index.value == 0 {
            sinfo!("2. Name is invalid for {}", wide_path.to_string_lossy());
        }
        name
    }

    pub unsafe fn new(string: &FString) -> FName {
        let mut ret = FName::default();
        (&raw mut ret).FName(string.as_ptr(), EFindName::Add);
        ret
    }
    pub unsafe fn find(string: &FString) -> FName {
        let mut ret = FName::default();
        (&raw mut ret).FName(string.as_ptr(), EFindName::Find);
        ret
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

pub type FnFNameToString = unsafe extern "system" fn(&FName, &mut FString);

pub type FnFNameCtorWchar = unsafe extern "system" fn(&mut FName, *const TCHAR, EFindName);
