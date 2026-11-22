//! Rust adapter from the native UObject model to Kismet reflection metadata.

use std::ptr;
use widestring::U16CString;
use unchained_macros_kismet::{FunctionMetadata, Reflection};
use crate::Chivalry2::Engine::{UClass, UObject, EFunctionFlags};
use crate::Chivalry2::Engine::Runtime::CoreUObject::UObject::UObjectGlobals::StaticFindObject;

pub(crate) struct EngineReflection;

// SAFETY: Lookup validates class/function objects and copies their native metadata.
// The caller supplies game-thread readiness and the matching executable environment.
unsafe impl Reflection for EngineReflection {
    unsafe fn find_function(&self, owner: &str, member: &str) -> Result<FunctionMetadata, String> {
        let path = U16CString::from_str(owner).map_err(|_| "Reflected class path contains NUL")?;
        let object = unsafe { StaticFindObject(ptr::null_mut(), ptr::null_mut(), path.as_ptr(), false) };
        let object = unsafe { UObject::from_ptr(object) }.ok_or_else(|| format!("Class {owner} is not loaded"))?;
        if !unsafe { object.inherits_from("Class") } {
            return Err(format!("{owner} is not a UClass"));
        }
        let class = (object as *const UObject).cast::<UClass>();
        let function = unsafe { UClass::find_function(class, member) }
            .ok_or_else(|| format!("Function {owner}::{member} was not found"))?;
        Ok(FunctionMetadata {
            is_native: unsafe { (*function).function_flags.contains(EFunctionFlags::FUNC_Native) },
            // Read storage directly; a null pointer must not become an invalid Rust fn value.
            exec_thunk: unsafe { ptr::addr_of!((*function).func).cast::<usize>().read() },
            num_parms: unsafe { (*function).num_parms },
            return_value_offset: unsafe { (*function).return_value_offset },
        })
    }
}
