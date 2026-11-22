//! Runtime address backend for reflected native C++ implementations.

use std::{num::NonZeroUsize, ptr};
use widestring::U16CString;
use crate::Chivalry2::Engine::{UClass, UObject, EFunctionFlags, FFrame};
use crate::Chivalry2::Engine::Runtime::CoreUObject::UObject::UObjectGlobals::StaticFindObject;

/// # Safety
/// Run during quiescent game-thread initialization after native classes are registered.
/// The declaration must match the recovered C++ ABI, not the exec-thunk ABI.
pub(crate) unsafe fn resolve_native(owner: &str, member: &str) -> Result<NonZeroUsize, String> {
    let path = U16CString::from_str(owner).map_err(|_| "Reflected class path contains NUL")?;
    let object = unsafe { StaticFindObject(ptr::null_mut(), ptr::null_mut(), path.as_ptr(), false) };
    let object = unsafe { UObject::from_ptr(object) }.ok_or_else(|| format!("Class {owner} is not loaded"))?;
    if !unsafe { object.inherits_from("Class") } {
        return Err(format!("{owner} is not a UClass"));
    }
    let class = (object as *const UObject).cast::<UClass>();
    let function = unsafe { UClass::find_function(class, member) }
        .ok_or_else(|| format!("Function {owner}::{member} was not found"))?;
    if !unsafe { &(*function).function_flags }.contains(EFunctionFlags::FUNC_Native) {
        return Err(format!("{owner}::{member} is not native"));
    }
    // Read raw storage so a missing native pointer never becomes an invalid Rust fn value.
    let thunk = unsafe { ptr::addr_of!((*function).func).cast::<usize>().read() };
    let step = FFrame::__UNCHAINED_BINDING_Step.binding().entry_address().ok_or("FFrame::Step is unbound")?.get();
    let explicit = FFrame::__UNCHAINED_BINDING_StepExplicitProperty.binding().entry_address().ok_or("FFrame::StepExplicitProperty is unbound")?.get();
    let target = unsafe { unchained_macros_kismet::native_body(thunk, *super::startup::BASE_ADDR, step, explicit) }?;
    sinfo!("Reflected {}::{}: thunk {:#x}, native {:#x}", owner, member, thunk, target);
    NonZeroUsize::new(target).ok_or_else(|| "Reflected native address is null".into())
}
