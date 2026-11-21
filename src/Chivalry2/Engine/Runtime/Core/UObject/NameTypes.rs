//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/UObject/NameTypes.h>.

use crate::Chivalry2::Engine::TCHAR;

use crate::Chivalry2::Engine::FString;
use crate::Chivalry2::Engine::UEHash;


use widestring::U16CString;

unchained_macros::define_custom_resolver!(FNameToString,
    delegate patternsleuth::resolvers::unreal::fname::FNameToString);

define_pattern_resolver!(
    FName__FName,
    Simple,
    ["48 89 5C 24 ?? 57 48 83 EC 30 48 8B D9 48 89 54 24 ??"]
);

#[unchained_macros::runtime_bindings]
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u32)]
pub enum EFindName {
    Find,
    Add,
    ReplaceNotSafeForThreading,
}

#[unchained_macros::runtime_bindings]
#[derive(Default, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
#[repr(C)]
pub struct FName {
    pub comparison_index: FNameEntryId,
    pub number: u32,
}

#[unchained_macros::runtime_bindings]
impl FName {
    /// # Safety
    /// The engine name table must be initialized and output must be writable.
    #[unchained_macros::bind_resolved_func(
        self::FNameToString
    )]
    pub unsafe fn ToString(&self, output: &mut FString) {}

    /// Call the constructor through the entry, never a
    /// copied trampoline: UE4SS may patch this entry after plugin initialization.
    ///
    /// # Safety
    /// Self must be writable native FName storage, text NUL-terminated UTF-16,
    /// and the engine name table initialized.
    #[bind_resolved_func(self::FName__FName)]
    pub unsafe fn FName(
        self: *mut Self,
        text: *const crate::Chivalry2::Engine::TCHAR,
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

#[unchained_macros::runtime_bindings]
#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
#[repr(C)]
pub struct FNameEntryId {
    pub value: u32,
}

impl std::fmt::Display for FName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut string = FString::new();
        unsafe {
            self.ToString(&mut string);
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

pub type FnFNameCtorWchar = unsafe extern "system" fn(&mut FName, *const TCHAR, EFindName);
