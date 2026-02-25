use crate::{globals, ue::FString};
use crate::resolvers::asset_registry::*;
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
impl FName {

    pub unsafe fn new(string: &FString) -> FName {
        let mut ret = FName::default();
        CALL_ORIGINAL_SAFE!(FNameCtorWchar(&mut ret, string.as_ptr(), EFindName::Add))
            .expect("Failed to construct FName (Add)");
        ret
    }
    pub unsafe fn find(string: &FString) -> FName {
        let mut ret = FName::default();
        CALL_ORIGINAL_SAFE!(FNameCtorWchar(&mut ret, string.as_ptr(), EFindName::Find))
            .expect("Failed to construct FName (Find)");
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
    /// Formats the `FName` using Unreal Engine's internal string conversion.
    ///
    /// # Safety
    /// This implementation is inherently unsafe because it calls `FName::ToString` via a 
    /// function pointer resolved at runtime from the game's memory.
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
