use unchained_macros_kismet::{kismet_exec_bindings, FunctionMetadata, Reflection, KismetContext, ExecEnvironment};
use unchained_macros_kismet::schema::*;
use unchained_macros_core::AddressResolver;

struct Library;
struct FString;
struct FText;
struct UObject;

#[kismet_exec_bindings("/Script/Fixture.Library")]
impl Library {
    #[dummy]
    pub unsafe fn PrintWarning(message: &FString) {}
    #[dummy]
    pub unsafe fn PrintText(message: &FText) {}
    #[dummy]
    pub unsafe fn Predicate(context: *mut UObject, flag: std::primitive::bool) -> bool {}
    #[dummy]
    pub unsafe fn NoArgs() {}
}

#[unchained_macros_core::bind_func_entry_listener(Library::PrintWarning)]
unsafe fn warning(_message: &FString) {}

#[unchained_macros_core::bind_func_entry_listener(Library::Predicate)]
unsafe fn predicate(_context: &*mut UObject, _flag: &bool) {}

struct Fixture { shared: bool }
unsafe impl Reflection for Fixture {
    unsafe fn find_function(&self, _: &str, _: &str) -> Result<FunctionMetadata, String> {
        Ok(FunctionMetadata { is_native: true, exec_thunk: 123, num_parms: 1, return_value_offset: u16::MAX })
    }
    unsafe fn find_exec_schema(&self, _: &str, _: &str) -> Result<FunctionSchema, String> {
        Ok(FunctionSchema {
            function_identity: 42, unique_exec_thunk: !self.shared,
            parameters: vec![PropertyMetadata {
                identity: 99, kind: PropertyKind::String, size: std::mem::size_of::<FString>(),
                array_dim: 1, flags: PARM | CONST_PARM, offset: 0,
            }],
        })
    }
}

#[test]
fn exec_resolver_uses_unique_wrapper_not_shared_dummy_body() {
    let context = unsafe { KismetContext::new(Fixture { shared: false }, ExecEnvironment {
        module_base: 1, frame_step: 2, frame_step_explicit: 3, frame_code_offset: 0,
    }) };
    let resolver = unsafe { unchained_macros_kismet::KismetExecResolver::new(
        "/Script/Fixture.Library", "PrintWarning", &[Parameter { kind: "StrProperty", size: std::mem::size_of::<FString>() }], None,
    ) };
    assert_eq!(unsafe { resolver.resolve(&context) }.unwrap().get(), 123);
    let _: unchained_macros_kismet::DummyExecMethodCannotBeDetoured = Library::__UNCHAINED_BINDING_PrintWarning;
}

#[test]
fn ambiguous_exec_wrapper_is_rejected_before_attachment() {
    let context = unsafe { KismetContext::new(Fixture { shared: true }, ExecEnvironment {
        module_base: 1, frame_step: 2, frame_step_explicit: 3, frame_code_offset: 0,
    }) };
    let resolver = unsafe { unchained_macros_kismet::KismetExecResolver::new(
        "/Script/Fixture.Library", "PrintWarning", &[Parameter { kind: "StrProperty", size: std::mem::size_of::<FString>() }], None,
    ) };
    assert!(unsafe { resolver.resolve(&context) }.unwrap_err().contains("Shared exec wrappers"));
}

#[derive(Default)]
struct Frame {
    evaluations: usize,
    original_calls: usize,
    cleanup: usize,
}
thread_local! {
    static RUNTIME_EVENTS: std::cell::RefCell<Vec<&'static str>> = const { std::cell::RefCell::new(Vec::new()) };
}
fn event(value: &'static str) { RUNTIME_EVENTS.with(|events| events.borrow_mut().push(value)); }
unsafe fn runtime_entry(context: &*mut UObject, flag: &bool) {
    assert_eq!(*context as usize, 0x123);
    assert!(*flag);
    event("entry");
    unsafe { invoke_reentrant_predicate(); }
}
unsafe fn invoke_reentrant_predicate() {
    let mut frame = Frame::default();
    let mut result = 0u8;
    unsafe {
        __UNCHAINED_EXEC_Library_Predicate_dispatch(std::ptr::null_mut(),
            (&mut frame as *mut Frame).cast(), (&mut result as *mut u8).cast());
    }
    assert_eq!((frame.evaluations, frame.original_calls, frame.cleanup, result), (2, 1, 1, 1));
}
unsafe extern "system" fn original_exec(_: *mut std::ffi::c_void, frame: *mut std::ffi::c_void, result: *mut std::ffi::c_void) {
    use unchained_macros_kismet::observation::observe_step;
    let state = unsafe { &mut *frame.cast::<Frame>() };
    state.original_calls += 1;
    let mut context = std::ptr::null_mut::<UObject>();
    let mut boolean = 0u32;
    unsafe {
        observe_step(frame, (&mut context as *mut *mut UObject).cast(), 11, || {
            state.evaluations += 1;
            context = 0x123usize as *mut UObject;
        });
        observe_step(frame, (&mut boolean as *mut u32).cast(), 12, || {
            state.evaluations += 1;
            boolean = 0x100; // Must read bool32, not its low byte.
        });
        event("body");
        result.cast::<u8>().write(1);
    }
    state.cleanup += 1;
    event("cleanup");
}
struct RuntimeReflection;
unsafe impl Reflection for RuntimeReflection {
    unsafe fn find_function(&self, _: &str, _: &str) -> Result<FunctionMetadata, String> {
        Ok(FunctionMetadata { is_native: true, exec_thunk: original_exec as *const () as usize,
            num_parms: 3, return_value_offset: 16 })
    }
    unsafe fn find_exec_schema(&self, _: &str, _: &str) -> Result<FunctionSchema, String> {
        Ok(FunctionSchema {
            function_identity: 42, unique_exec_thunk: true,
            parameters: vec![
                PropertyMetadata { identity: 11, kind: PropertyKind::Object, size: 8, array_dim: 1, flags: PARM, offset: 0 },
                PropertyMetadata { identity: 12, kind: PropertyKind::Bool, size: 1, array_dim: 1, flags: PARM, offset: 8 },
                PropertyMetadata { identity: 13, kind: PropertyKind::Bool, size: 1, array_dim: 1, flags: PARM | RETURN_PARM, offset: 16 },
            ],
        })
    }
}

#[test]
fn generated_typed_dispatch_preserves_vm_bool_result_original_and_cleanup() {
    let context = unsafe { KismetContext::new(RuntimeReflection, ExecEnvironment {
        module_base: 1, frame_step: 2, frame_step_explicit: 3, frame_code_offset: 0,
    }) };
    let address = unsafe { __UNCHAINED_EXEC_Library_Predicate_RESOLVER.implementation.resolve(&context) }.unwrap();
    unsafe { __UNCHAINED_EXEC_Library_Predicate_BINDING.initialize_address(address) }.unwrap();
    unsafe { Library::__UNCHAINED_LISTENERS_Predicate.entry.register(runtime_entry, true, || true) }.unwrap();
    let mut frame = Frame::default();
    let mut result = 0u8;
    unsafe {
        __UNCHAINED_EXEC_Library_Predicate_dispatch(std::ptr::null_mut(),
            (&mut frame as *mut Frame).cast(), (&mut result as *mut u8).cast());
    }
    assert_eq!((frame.evaluations, frame.original_calls, frame.cleanup, result), (2, 1, 1, 1));
    RUNTIME_EVENTS.with(|events| assert_eq!(*events.borrow(),
        ["entry", "body", "cleanup", "body", "cleanup"]));
    std::thread::spawn(|| {
        let mut frame = Frame::default();
        let mut result = 0u8;
        unsafe {
            __UNCHAINED_EXEC_Library_Predicate_dispatch(std::ptr::null_mut(),
                (&mut frame as *mut Frame).cast(), (&mut result as *mut u8).cast());
        }
        assert_eq!((frame.evaluations, frame.original_calls, frame.cleanup, result), (2, 1, 1, 1));
        RUNTIME_EVENTS.with(|events| assert_eq!(*events.borrow(), ["body", "cleanup"]));
    }).join().unwrap();
}

thread_local! {
    static NOARGS_ORIGINALS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static NOARGS_ENTRIES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
unsafe fn invoke_noargs() {
    unsafe {
        __UNCHAINED_EXEC_Library_NoArgs_dispatch(
            std::ptr::null_mut(), std::ptr::dangling_mut(), std::ptr::null_mut());
    }
}
unsafe fn noargs_reentrant_entry() {
    NOARGS_ENTRIES.with(|count| count.set(count.get() + 1));
    unsafe { invoke_noargs(); } // Unconditional: the descriptor must stop recursion.
}
unsafe extern "system" fn noargs_original(_: *mut std::ffi::c_void, _: *mut std::ffi::c_void, result: *mut std::ffi::c_void) {
    assert!(result.is_null(), "Original receives untouched native void result storage");
    NOARGS_ORIGINALS.with(|count| count.set(count.get() + 1));
}
struct NoArgsReflection;
unsafe impl Reflection for NoArgsReflection {
    unsafe fn find_function(&self, _: &str, _: &str) -> Result<FunctionMetadata, String> {
        Ok(FunctionMetadata { is_native: true, exec_thunk: noargs_original as *const () as usize,
            num_parms: 0, return_value_offset: u16::MAX })
    }
    unsafe fn find_exec_schema(&self, _: &str, _: &str) -> Result<FunctionSchema, String> {
        Ok(FunctionSchema { function_identity: 43, unique_exec_thunk: true, parameters: vec![] })
    }
}

#[test]
fn noargs_entry_unconditional_reentry_suppresses_only_nested_observers() {
    let context = unsafe { KismetContext::new(NoArgsReflection, ExecEnvironment {
        module_base: 1, frame_step: 2, frame_step_explicit: 3, frame_code_offset: 0,
    }) };
    let address = unsafe { __UNCHAINED_EXEC_Library_NoArgs_RESOLVER.implementation.resolve(&context) }.unwrap();
    unsafe { __UNCHAINED_EXEC_Library_NoArgs_BINDING.initialize_address(address) }.unwrap();
    unsafe { Library::__UNCHAINED_LISTENERS_NoArgs.entry.register(noargs_reentrant_entry, true, || true) }.unwrap();
    unsafe { invoke_noargs(); }
    NOARGS_ENTRIES.with(|count| assert_eq!(count.get(), 1));
    NOARGS_ORIGINALS.with(|count| assert_eq!(count.get(), 2));
    std::thread::spawn(|| {
        unsafe { invoke_noargs(); }
        NOARGS_ORIGINALS.with(|count| assert_eq!(count.get(), 1));
        NOARGS_ENTRIES.with(|count| assert_eq!(count.get(), 0));
    }).join().unwrap();
}
