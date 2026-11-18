use std::mem::MaybeUninit;

use super::*;

#[derive(Debug)]
#[repr(C)]
pub struct FFrame {
    pub base: FOutputDevice,
    pub node: *const c_void,
    pub object: *mut UObject,
    pub code: *const c_void,
    pub locals: *const c_void,
    pub most_recent_property: *const FProperty,
    pub most_recent_property_address: *const c_void,
    pub flow_stack: [u8; 0x30],
    pub previous_frame: *const FFrame,
    pub out_parms: *const c_void,
    pub property_chain_for_compiled_in: *const FField,
    pub current_native_function: *const c_void,
    pub b_array_context_failed: bool,
}

/// The ABI of every UFunction native exec thunk:
/// `void exec(UObject* Context, FFrame& Stack, void* Result)`.
pub type FNativeExec = unsafe extern "system" fn(*mut UObject, *mut FFrame, *mut c_void);

/// A reflected UClass whose native UFunction thunks can be resolved by name at runtime.
///
/// Resolution reads the class's `RegisterNatives` table (a `{ name -> address }` map built during
/// `init_globals`), i.e. Unreal's own reflection data — not a byte signature. Each class has its
/// own table, so each needs its own `globals()` resolver method; `kismet_native_class!` only wraps
/// an existing one.
pub trait KismetNativeClass {
    const CLASS_NAME: &'static str;
    const VIRTUAL_DISPATCH_PATH: Option<&'static str> = None;
    /// Absolute address of the named native's exec thunk, or `None` if it was not resolved.
    unsafe fn resolve(native: &str) -> Option<usize>;

    /// Supply a receiver with the vtable used by this binding's virtual exec thunks.
    /// A class path opts in to using its existing class default object. Interface bindings
    /// can override this method to supply the appropriate live interface pointer.
    unsafe fn virtual_receiver() -> Option<*const usize> {
        super::exec_thunk::default_object_receiver(Self::VIRTUAL_DISPATCH_PATH?)
    }

    unsafe fn resolve_hook_target(thunk: usize) -> Option<usize> {
        super::exec_thunk::hook_native_of(thunk, || Self::virtual_receiver())
    }
}

impl FFrame {
    pub unsafe fn arg<T: Sized>(self: &mut FFrame) -> T {
        let mut value: MaybeUninit<T> = MaybeUninit::zeroed();
        let ptr = value.as_mut_ptr() as *mut _;

        if self.code.is_null() {
            let cur = self.property_chain_for_compiled_in;
            self.property_chain_for_compiled_in = (*cur).next;
            globals().fframe_step_explicit_property()(self, ptr, cur as *const FProperty);
        } else {
            globals().fframe_step()(self, self.object, ptr);
        }

        value.assume_init()
    }

    /// Advance the instruction pointer past `EX_EndFunctionParms`, mirroring the engine's
    /// `P_FINISH` macro (`Stack->Code += (Stack->Code != 0)`). This is a no-op on the
    /// compiled-in/native path where `Code` is null. Call it exactly once, after stepping every
    /// declared parameter of the function, so the VM resumes from the correct bytecode position.
    pub unsafe fn finish(self: &mut FFrame) {
        if !self.code.is_null() {
            self.code = (self.code as *const u8).add(1) as *const c_void;
        }
    }
}
