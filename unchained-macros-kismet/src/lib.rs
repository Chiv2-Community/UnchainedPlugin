#![doc = include_str!("../README.md")]

pub use unchained_macros_kismet_impl::kismet_runtime_bindings;
pub mod observation;
pub mod schema;
mod exec_binding;
pub use exec_binding::{KismetExecResolver, DummyExecTarget};
use std::{num::NonZeroUsize, sync::{Mutex, OnceLock}};

/// Copied UFunction metadata; the exec address is raw storage, not a Rust fn value.
#[derive(Clone, Copy, Debug)]
pub struct FunctionMetadata {
    pub is_native: bool,
    pub exec_thunk: usize,
    /// Includes the return parameter, when present.
    pub num_parms: u8,
    /// `u16::MAX` means there is no return parameter.
    pub return_value_offset: u16,
}

/// Layout-specific access to the host's native reflection model.
///
/// # Safety
/// Implementations must return accurate metadata for the requested owner/member.
/// Non-null exec addresses must remain valid in the module supplied to KismetContext.
pub unsafe trait Reflection {
    /// Find a UFunction on the class or its superclasses and copy its metadata.
    ///
    /// # Safety
    /// Call on the game thread after the host's reflection prerequisites are ready.
    unsafe fn find_function(&self, owner: &str, member: &str) -> Result<FunctionMetadata, String>;

    /// Copy parameter identity, flags, kind and layout from the same UFunction.
    ///
    /// # Safety
    /// The same game-thread/readiness contract as `find_function` applies.
    unsafe fn find_exec_schema(&self, _owner: &str, _member: &str) -> Result<schema::FunctionSchema, String> {
        Err("Reflection adapter does not provide exec observation schemas".into())
    }
}

/// Verified executable addresses and the host's FFrame layout.
#[derive(Clone, Copy, Debug)]
pub struct ExecEnvironment {
    pub module_base: usize,
    /// Native entries, never binding wrappers or detour trampolines.
    pub frame_step: usize,
    pub frame_step_explicit: usize,
    pub frame_code_offset: usize,
}

/// Game-thread reflection adapter and executable environment.
pub struct KismetContext {
    reflection: Box<dyn Reflection>,
    environment: ExecEnvironment,
    observation_thread: u32,
    _game_thread: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl KismetContext {
    /// # Safety
    /// Construct and use on the game thread once reflection is ready.
    /// Native callers must remain quiescent until bindings and hooks are published.
    /// The environment must describe the adapter's module and verified frame-step entries.
    pub unsafe fn new(reflection: impl Reflection + 'static, environment: ExecEnvironment) -> Self {
        Self { reflection: Box::new(reflection), environment,
            observation_thread: exec_binding::current_thread_id(),
            _game_thread: std::marker::PhantomData }
    }

    unsafe fn resolve_native(&self, owner: &str, member: &str) -> Result<NonZeroUsize, String> {
        let function = unsafe { self.reflection.find_function(owner, member) }?;
        let arguments = function.arguments().map_err(|error| format!("{owner}::{member}: {error}"))?;
        let target = unsafe { decode_native(function.exec_thunk, arguments, self.environment) }?;
        log::info!("Reflected {}::{}: thunk {:#x}, native {:#x}", owner, member, function.exec_thunk, target);
        NonZeroUsize::new(target).ok_or_else(|| "Reflected native address is null".into())
    }
}

impl FunctionMetadata {
    fn arguments(&self) -> Result<u8, String> {
        if !self.is_native { return Err("Reflected function is not native".into()); }
        if self.exec_thunk == 0 { return Err("Reflected exec thunk is null".into()); }
        self.num_parms.checked_sub(u8::from(self.return_value_offset != u16::MAX))
            .ok_or_else(|| "Invalid reflected parameter count".into())
    }
}

unsafe fn decode_native(thunk: usize, arguments: u8, environment: ExecEnvironment) -> Result<usize, String> {
    if environment.module_base == 0 || environment.frame_step == 0 || environment.frame_step_explicit == 0 {
        return Err("Kismet executable environment contains a null address".into());
    }
    let parameters = if arguments == 0 {
        ExecThunkParameters::NoArguments { frame_code_offset: environment.frame_code_offset }
    } else {
        ExecThunkParameters::Stepped
    };
    unsafe { native_body(thunk, environment.module_base, environment.frame_step, environment.frame_step_explicit, parameters) }
}

pub struct KismetReflectionResolver {
    pub owner: &'static str,
    pub member: &'static str,
    address: OnceLock<NonZeroUsize>,
    lock: Mutex<()>,
}
impl KismetReflectionResolver {
    pub const fn new(owner: &'static str, member: &'static str) -> Self {
        Self { owner, member, address: OnceLock::new(), lock: Mutex::new(()) }
    }
}
impl unchained_macros_core::AddressResolver for KismetReflectionResolver {
    type Context = KismetContext;
    unsafe fn resolve(&self, context: &KismetContext) -> Result<NonZeroUsize, String> {
        let _guard = self.lock.lock().map_err(|_| "Kismet resolver lock poisoned")?;
        if let Some(address) = self.address.get() { return Ok(*address); }
        let address = unsafe { context.resolve_native(self.owner, self.member)? };
        self.address.set(address).map_err(|_| "Kismet resolver was already published")?;
        Ok(address)
    }
}

mod exec_thunk;
pub use exec_thunk::{native_body, ExecThunkParameters};

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};
    use unchained_macros_core::AddressResolver;

    fn function(num_parms: u8, return_value_offset: u16) -> FunctionMetadata {
        FunctionMetadata { is_native: true, exec_thunk: 1, num_parms, return_value_offset }
    }

    #[test_case::test_case(0, u16::MAX, 0; "no parameters")]
    #[test_case::test_case(1, 0, 0; "return only")]
    #[test_case::test_case(3, 16, 2; "two arguments and return")]
    #[test_case::test_case(2, u16::MAX, 2; "two arguments without return")]
    fn return_storage_is_not_an_argument(num_parms: u8, return_offset: u16, arguments: u8) {
        assert_eq!(function(num_parms, return_offset).arguments().unwrap(), arguments);
    }

    #[test]
    fn invalid_function_metadata_is_rejected() {
        let mut info = function(1, 0);
        info.is_native = false;
        assert!(info.arguments().unwrap_err().contains("not native"));
        info.is_native = true;
        info.exec_thunk = 0;
        assert!(info.arguments().unwrap_err().contains("null"));
        info.exec_thunk = 1;
        info.num_parms = 0;
        assert!(info.arguments().unwrap_err().contains("parameter count"));
    }

    struct MetadataReflection {
        reads: Rc<Cell<usize>>,
        ready: Rc<Cell<bool>>,
    }
    // SAFETY: Only copied invalid metadata is returned; tests reject it before decoding.
    unsafe impl Reflection for MetadataReflection {
        unsafe fn find_function(&self, owner: &str, member: &str) -> Result<FunctionMetadata, String> {
            assert_eq!((owner, member), ("/Script/Fixture.Native", "Call"));
            self.reads.set(self.reads.get() + 1);
            if !self.ready.get() { return Err("class not loaded".into()); }
            Ok(FunctionMetadata { exec_thunk: 0, ..function(1, 0) })
        }
    }

    #[test]
    fn failed_lookup_and_metadata_validation_remain_retryable() {
        let reads = Rc::new(Cell::new(0));
        let ready = Rc::new(Cell::new(false));
        // An unusable environment ensures metadata errors never reach native decoding.
        let context = KismetContext {
            reflection: Box::new(MetadataReflection { reads: reads.clone(), ready: ready.clone() }),
            environment: ExecEnvironment { module_base: 0, frame_step: 0, frame_step_explicit: 0, frame_code_offset: 0 },
            observation_thread: exec_binding::current_thread_id(),
            _game_thread: std::marker::PhantomData,
        };
        let resolver = KismetReflectionResolver::new("/Script/Fixture.Native", "Call");
        for expected in ["class not loaded", "class not loaded", "null", "null"] {
            if reads.get() == 2 { ready.set(true); }
            assert!(unsafe { resolver.resolve(&context) }.unwrap_err().contains(expected));
            assert!(resolver.address.get().is_none());
        }
        assert_eq!(reads.get(), 4);
    }

    #[test]
    fn null_environment_addresses_fail_before_native_access() {
        let environment = ExecEnvironment { module_base: 1, frame_step: 2, frame_step_explicit: 3, frame_code_offset: 0 };
        for environment in [
            ExecEnvironment { module_base: 0, ..environment },
            ExecEnvironment { frame_step: 0, ..environment },
            ExecEnvironment { frame_step_explicit: 0, ..environment },
        ] {
            assert!(unsafe { decode_native(1, 0, environment) }.unwrap_err().contains("null address"));
        }
    }
}
