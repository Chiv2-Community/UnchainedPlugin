//! Entry-only observation leaves original result storage and ownership untouched.
use std::{
    ffi::c_void,
    mem::{size_of, MaybeUninit},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use unchained_macros_core::AddressResolver;
use unchained_macros_kismet::schema::*;
use unchained_macros_kismet::{kismet_runtime_bindings, ExecEnvironment, FunctionMetadata, KismetContext, Reflection};

static STRING_DROPS: AtomicUsize = AtomicUsize::new(0);
static TEXT_DROPS: AtomicUsize = AtomicUsize::new(0);
static OBSERVED: AtomicUsize = AtomicUsize::new(0);
static NULL_ORIGINALS: AtomicUsize = AtomicUsize::new(0);
static NULL_OBSERVED: AtomicUsize = AtomicUsize::new(0);
static MISALIGNED_OBSERVED: AtomicUsize = AtomicUsize::new(0);
static MISALIGNED_ORIGINALS: AtomicUsize = AtomicUsize::new(0);
struct FString(String);
impl Drop for FString {
    fn drop(&mut self) {
        STRING_DROPS.fetch_add(1, Ordering::SeqCst);
    }
}

struct FText(Arc<()>);
impl Drop for FText {
    fn drop(&mut self) {
        TEXT_DROPS.fetch_add(1, Ordering::SeqCst);
    }
}

#[repr(C)]
struct FLinearColor {
    red: f32,
    green: f32,
    blue: f32,
    alpha: f32,
}

struct Library;
include!("fixtures/exec_access.rs");
#[kismet_runtime_bindings("/Script/Fixture.Results")]
impl Library {
    exec_access!(
        __UNCHAINED_EXEC_Library_StringResult_RESOLVER,
        __UNCHAINED_EXEC_Library_StringResult_BINDING,
        __UNCHAINED_EXEC_Library_StringResult_dispatch
    );
    exec_access!(
        __UNCHAINED_EXEC_Library_TextResult_RESOLVER,
        __UNCHAINED_EXEC_Library_TextResult_BINDING,
        __UNCHAINED_EXEC_Library_TextResult_dispatch
    );
    exec_access!(
        __UNCHAINED_EXEC_Library_ColorResult_RESOLVER,
        __UNCHAINED_EXEC_Library_ColorResult_BINDING,
        __UNCHAINED_EXEC_Library_ColorResult_dispatch
    );
    exec_access!(
        __UNCHAINED_EXEC_Library_NullStringResult_RESOLVER,
        __UNCHAINED_EXEC_Library_NullStringResult_BINDING,
        __UNCHAINED_EXEC_Library_NullStringResult_dispatch
    );
    exec_access!(
        __UNCHAINED_EXEC_Library_MisalignedColorResult_RESOLVER,
        __UNCHAINED_EXEC_Library_MisalignedColorResult_BINDING,
        __UNCHAINED_EXEC_Library_MisalignedColorResult_dispatch
    );
    #[dummy]
    unsafe fn StringResult() -> FString {}
    #[dummy]
    unsafe fn TextResult() -> FText {}
    #[dummy]
    unsafe fn ColorResult() -> FLinearColor {}
    #[dummy]
    unsafe fn NullStringResult() -> FString {}
    #[dummy]
    unsafe fn MisalignedColorResult() -> FLinearColor {}
}

#[unchained_macros_core::bind_func_entry_listener(Library::StringResult)]
unsafe fn string_result() {
    assert_eq!(STRING_DROPS.load(Ordering::SeqCst), 0);
    OBSERVED.fetch_add(1, Ordering::SeqCst);
}

#[unchained_macros_core::bind_func_entry_listener(Library::TextResult)]
unsafe fn text_result() {
    assert_eq!(TEXT_DROPS.load(Ordering::SeqCst), 0);
    OBSERVED.fetch_add(1, Ordering::SeqCst);
}

#[unchained_macros_core::bind_func_entry_listener(Library::ColorResult)]
unsafe fn color_result() {
    OBSERVED.fetch_add(1, Ordering::SeqCst);
}

#[unchained_macros_core::bind_func_entry_listener(Library::NullStringResult)]
unsafe fn null_result() {
    NULL_OBSERVED.fetch_add(1, Ordering::SeqCst);
}

#[unchained_macros_core::bind_func_entry_listener(Library::MisalignedColorResult)]
unsafe fn misaligned_result() {
    MISALIGNED_OBSERVED.fetch_add(1, Ordering::SeqCst);
}

unsafe extern "system" fn string_original(_: *mut c_void, _: *mut c_void, result: *mut c_void) {
    unsafe {
        result.cast::<FString>().write(FString("caller-owned".into()));
    }
}

unsafe extern "system" fn text_original(_: *mut c_void, _: *mut c_void, result: *mut c_void) {
    unsafe {
        result.cast::<FText>().write(FText(Arc::new(())));
    }
}

unsafe extern "system" fn color_original(_: *mut c_void, _: *mut c_void, result: *mut c_void) {
    unsafe {
        result.cast::<FLinearColor>().write(FLinearColor {
            red: 1.0,
            green: 0.5,
            blue: 0.25,
            alpha: 1.0,
        });
    }
}

unsafe extern "system" fn null_original(_: *mut c_void, _: *mut c_void, result: *mut c_void) {
    // Invalid caller storage is tolerated only by this failure-path fixture.
    assert!(result.is_null());
    NULL_ORIGINALS.fetch_add(1, Ordering::SeqCst);
}

unsafe extern "system" fn unaligned_original(_: *mut c_void, _: *mut c_void, result: *mut c_void) {
    unsafe {
        result.cast::<FLinearColor>().write_unaligned(FLinearColor {
            red: 1.0,
            green: 0.5,
            blue: 0.25,
            alpha: 1.0,
        });
    }

    MISALIGNED_ORIGINALS.fetch_add(1, Ordering::SeqCst);
}

struct Fixture;
fn native(member: &str) -> (usize, PropertyKind, usize) {
    match member {
        "StringResult" => (
            string_original as *const () as usize,
            PropertyKind::String,
            size_of::<FString>(),
        ),
        "TextResult" => (
            text_original as *const () as usize,
            PropertyKind::Text,
            size_of::<FText>(),
        ),
        "ColorResult" => (
            color_original as *const () as usize,
            PropertyKind::Struct("LinearColor".into()),
            size_of::<FLinearColor>(),
        ),
        "NullStringResult" => (
            null_original as *const () as usize,
            PropertyKind::String,
            size_of::<FString>(),
        ),
        "MisalignedColorResult" => (
            unaligned_original as *const () as usize,
            PropertyKind::Struct("LinearColor".into()),
            size_of::<FLinearColor>(),
        ),
        _ => unreachable!(),
    }
}

unsafe impl Reflection for Fixture {
    unsafe fn find_function(&self, _: &str, member: &str) -> Result<FunctionMetadata, String> {
        Ok(FunctionMetadata {
            is_native: true,
            exec_thunk: native(member).0,
            num_parms: 1,
            return_value_offset: 0,
        })
    }

    unsafe fn find_exec_schema(&self, _: &str, member: &str) -> Result<FunctionSchema, String> {
        let (identity, kind, size) = native(member);
        Ok(FunctionSchema {
            function_identity: identity,
            unique_exec_thunk: true,
            parameters: vec![PropertyMetadata {
                identity,
                kind,
                size,
                array_dim: 1,
                flags: PARM | RETURN_PARM,
                offset: 0,
            }],
        })
    }
}

fn context() -> KismetContext {
    unsafe {
        KismetContext::new(
            Fixture,
            ExecEnvironment {
                module_base: 1,
                frame_step: 2,
                frame_step_explicit: 3,
                frame_code_offset: 0,
            },
        )
    }
}

#[test]
fn caller_keeps_string_text_and_color_results_unchanged_and_owns_cleanup() {
    let context = context();
    macro_rules! initialize {
        ($resolver:ident, $binding:ident, $descriptor:ident, $callback:ident) => {
            let address = unsafe { Library::$resolver.implementation.resolve(&context) }.unwrap();
            unsafe { Library::$binding.initialize_address(address) }.unwrap();
            unsafe { Library::$descriptor.entry.register($callback, true, || true) }.unwrap();
        };
    }

    initialize!(
        __UNCHAINED_EXEC_Library_StringResult_RESOLVER,
        __UNCHAINED_EXEC_Library_StringResult_BINDING,
        __UNCHAINED_TARGET_StringResult,
        string_result
    );
    initialize!(
        __UNCHAINED_EXEC_Library_TextResult_RESOLVER,
        __UNCHAINED_EXEC_Library_TextResult_BINDING,
        __UNCHAINED_TARGET_TextResult,
        text_result
    );
    initialize!(
        __UNCHAINED_EXEC_Library_ColorResult_RESOLVER,
        __UNCHAINED_EXEC_Library_ColorResult_BINDING,
        __UNCHAINED_TARGET_ColorResult,
        color_result
    );
    let mut string = MaybeUninit::<FString>::uninit();
    let mut text = MaybeUninit::<FText>::uninit();
    let mut color = MaybeUninit::<FLinearColor>::uninit();
    unsafe {
        (Library::__UNCHAINED_EXEC_Library_StringResult_dispatch)(
            std::ptr::null_mut(),
            std::ptr::dangling_mut(),
            string.as_mut_ptr().cast(),
        );
        (Library::__UNCHAINED_EXEC_Library_TextResult_dispatch)(
            std::ptr::null_mut(),
            std::ptr::dangling_mut(),
            text.as_mut_ptr().cast(),
        );
        (Library::__UNCHAINED_EXEC_Library_ColorResult_dispatch)(
            std::ptr::null_mut(),
            std::ptr::dangling_mut(),
            color.as_mut_ptr().cast(),
        );
        let string = string.assume_init();
        let text = text.assume_init();
        let color = color.assume_init();
        assert_eq!(string.0, "caller-owned");
        assert_eq!(Arc::strong_count(&text.0), 1);
        assert_eq!(color.green, 0.5);
        assert_eq!((color.red, color.blue, color.alpha), (1.0, 0.25, 1.0));
        assert_eq!(OBSERVED.load(Ordering::SeqCst), 3);
        assert_eq!(STRING_DROPS.load(Ordering::SeqCst), 0);
        assert_eq!(TEXT_DROPS.load(Ordering::SeqCst), 0);
        drop(string);
        drop(text);
    }

    assert_eq!(STRING_DROPS.load(Ordering::SeqCst), 1);
    assert_eq!(TEXT_DROPS.load(Ordering::SeqCst), 1);
}

#[test]
fn null_result_is_forwarded_without_forming_native_owner_reference() {
    let context = context();
    let address = unsafe {
        Library::__UNCHAINED_EXEC_Library_NullStringResult_RESOLVER
            .implementation
            .resolve(&context)
    }
    .unwrap();
    unsafe { Library::__UNCHAINED_EXEC_Library_NullStringResult_BINDING.initialize_address(address) }.unwrap();
    unsafe {
        Library::__UNCHAINED_TARGET_NullStringResult
            .entry
            .register(null_result, true, || true)
    }
    .unwrap();
    unsafe {
        (Library::__UNCHAINED_EXEC_Library_NullStringResult_dispatch)(
            std::ptr::null_mut(),
            std::ptr::dangling_mut(),
            std::ptr::null_mut(),
        );
    }

    assert_eq!(NULL_ORIGINALS.load(Ordering::SeqCst), 1);
    assert_eq!(NULL_OBSERVED.load(Ordering::SeqCst), 1);
}

#[test]
fn misaligned_result_is_forwarded_without_typed_borrow() {
    #[repr(align(16))]
    struct Buffer([u8; 32]);
    let context = context();
    let address = unsafe {
        Library::__UNCHAINED_EXEC_Library_MisalignedColorResult_RESOLVER
            .implementation
            .resolve(&context)
    }
    .unwrap();
    unsafe { Library::__UNCHAINED_EXEC_Library_MisalignedColorResult_BINDING.initialize_address(address) }.unwrap();
    unsafe {
        Library::__UNCHAINED_TARGET_MisalignedColorResult
            .entry
            .register(misaligned_result, true, || true)
    }
    .unwrap();
    let mut buffer = Buffer([0; 32]);
    let result = unsafe { buffer.0.as_mut_ptr().add(1).cast::<c_void>() };
    unsafe {
        (Library::__UNCHAINED_EXEC_Library_MisalignedColorResult_dispatch)(
            std::ptr::null_mut(),
            std::ptr::dangling_mut(),
            result,
        );
    }

    assert_eq!(MISALIGNED_ORIGINALS.load(Ordering::SeqCst), 1);
    assert_eq!(MISALIGNED_OBSERVED.load(Ordering::SeqCst), 1);
    let result = unsafe { result.cast::<FLinearColor>().read_unaligned() };
    assert_eq!(result.green, 0.5);
}
