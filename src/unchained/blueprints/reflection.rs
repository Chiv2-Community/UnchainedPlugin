//! Rust adapter from the native UObject model to Kismet reflection metadata.

use std::ptr;
use widestring::U16CString;
use unchained_macros_kismet::{FunctionMetadata, Reflection};
use unchained_macros_kismet::schema::{FunctionSchema, PropertyKind, PropertyMetadata, PARM};
use crate::Chivalry2::Engine::{UClass, UObject, EFunctionFlags};
use crate::Chivalry2::Engine::{UFunction, FProperty, GUObjectArray};
use crate::Chivalry2::Engine::Runtime::CoreUObject::UObject::UObjectGlobals::StaticFindObject;

pub(crate) struct EngineReflection;

unsafe fn find(owner: &str, member: &str) -> Result<*const UFunction, String> {
    let path = U16CString::from_str(owner).map_err(|_| "Reflected class path contains NUL")?;
    let object = unsafe { StaticFindObject(ptr::null_mut(), ptr::null_mut(), path.as_ptr(), false) };
    let object = unsafe { UObject::from_ptr(object) }.ok_or_else(|| format!("Class {owner} is not loaded"))?;
    if !unsafe { object.inherits_from("Class") } { return Err(format!("{owner} is not a UClass")); }
    unsafe { UClass::find_function((object as *const UObject).cast::<UClass>(), member) }
        .ok_or_else(|| format!("Function {owner}::{member} was not found"))
}

// SAFETY: Lookup validates class/function objects and copies their native metadata.
// The caller supplies game-thread readiness and the matching executable environment.
unsafe impl Reflection for EngineReflection {
    unsafe fn find_function(&self, owner: &str, member: &str) -> Result<FunctionMetadata, String> {
        let function = unsafe { find(owner, member) }?;
        Ok(FunctionMetadata {
            is_native: unsafe { (*function).function_flags.contains(EFunctionFlags::FUNC_Native) },
            // Read storage directly; a null pointer must not become an invalid Rust fn value.
            exec_thunk: unsafe { ptr::addr_of!((*function).func).cast::<usize>().read() },
            num_parms: unsafe { (*function).num_parms },
            return_value_offset: unsafe { (*function).return_value_offset },
        })
    }

    unsafe fn find_exec_schema(&self, owner: &str, member: &str) -> Result<FunctionSchema, String> {
        let function = unsafe { find(owner, member) }?;
        let mut field = unsafe { (*function).ustruct.child_properties };
        let mut parameters = Vec::new();
        let mut seen = std::collections::HashSet::new();
        while !field.is_null() {
            if !seen.insert(field as usize) || seen.len() > 4096 {
                return Err("Cyclic/excessive reflected parameter chain".into());
            }
            let property = unsafe { &*field.cast::<FProperty>() };
            if property.property_flags & PARM != 0 {
                let class = unsafe { (*field).class_private };
                if class.is_null() { return Err("Reflected parameter has no field class".into()); }
                let name = unsafe { (*class).name.to_string() };
                let kind = match name.as_str() {
                    "BoolProperty" => PropertyKind::Bool,
                    "ByteProperty" => PropertyKind::Byte,
                    "IntProperty" => PropertyKind::Int,
                    "FloatProperty" => PropertyKind::Float,
                    "ObjectProperty" => PropertyKind::Object,
                    "StrProperty" => PropertyKind::String,
                    "TextProperty" => PropertyKind::Text,
                    "StructProperty" => {
                        let strukt = unsafe {
                            field.cast::<crate::Chivalry2::Engine::FStructProperty>().script_struct()
                        };
                        if strukt.is_null() { return Err("Struct parameter has no script struct".into()); }
                        PropertyKind::Struct(unsafe { (*strukt).ufield.uobject.name() })
                    }
                    _ => PropertyKind::Unsupported(name),
                };
                parameters.push(PropertyMetadata {
                    identity: field as usize, kind,
                    size: usize::try_from(property.element_size).map_err(|_| "Negative parameter size")?,
                    array_dim: property.array_dim, flags: property.property_flags,
                    offset: usize::try_from(property.offset_internal).map_err(|_| "Negative parameter offset")?,
                });
            }
            field = unsafe { (*field).next };
        }
        let thunk = unsafe { ptr::addr_of!((*function).func).cast::<usize>().read() };
        // Fail closed for shared wrappers. FFrame::node is not assumed to name
        // the callee, and current_native_function has no verified contract here.
        let mut unique_exec_thunk = true;
        let objects: Vec<*const UObject> = {
            let registry = unsafe { &*GUObjectArray.read() };
            let objects = registry.objects();
            objects.iter().flatten().map(|object| (object as *const crate::Chivalry2::Engine::UObjectBase).cast()).collect()
        };
        let mut function_classes = std::collections::HashMap::new();
        for object in objects {
            if object == function.cast() { continue; }
            let Some(live) = (unsafe { UObject::from_ptr(object.cast_mut()) }) else { continue; };
            let class = live.uobject_base_utility.uobject_base.class_private as usize;
            let is_function = if let Some(value) = function_classes.get(&class) {
                *value
            } else {
                let value = unsafe { live.inherits_from("Function") };
                function_classes.insert(class, value);
                value
            };
            if is_function {
                let other = object.cast::<UFunction>();
                if unsafe { (*other).function_flags.contains(EFunctionFlags::FUNC_Native) }
                    && unsafe { ptr::addr_of!((*other).func).cast::<usize>().read() } == thunk
                {
                    unique_exec_thunk = false;
                    break;
                }
            }
        }
        Ok(FunctionSchema { function_identity: function as usize, unique_exec_thunk, parameters })
    }
}
