//! Entry listeners borrow VM owners without taking or duplicating ownership.
use std::{
    ffi::c_void,
    mem::size_of,
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
static ORIGINALS: AtomicUsize = AtomicUsize::new(0);
static ENTRIES: AtomicUsize = AtomicUsize::new(0);
struct FString(String);
impl Drop for FString {
    fn drop(&mut self) {
        STRING_DROPS.fetch_add(1, Ordering::SeqCst);
    }
}

struct FText(Arc<String>);
impl Drop for FText {
    fn drop(&mut self) {
        TEXT_DROPS.fetch_add(1, Ordering::SeqCst);
    }
}

struct Library;
include!("fixtures/exec_access.rs");
#[kismet_runtime_bindings("/Script/Fixture.Inputs")]
impl Library {
    exec_access!(
        __UNCHAINED_EXEC_Library_Owners_RESOLVER,
        __UNCHAINED_EXEC_Library_Owners_BINDING,
        __UNCHAINED_EXEC_Library_Owners_dispatch
    );
    #[dummy]
    unsafe fn Owners(string: &FString, text: &FText) {}
}

#[unchained_macros_core::bind_func_entry_listener(Library::Owners)]
unsafe fn owners_entry(string: &FString, text: &FText) {
    assert_eq!(string.0, "entry-string");
    assert_eq!(*text.0, "entry-text");
    assert_eq!(Arc::strong_count(&text.0), 1, "Observation must not retain ownership");
    assert_eq!(STRING_DROPS.load(Ordering::SeqCst), 0);
    assert_eq!(TEXT_DROPS.load(Ordering::SeqCst), 0);
    ENTRIES.fetch_add(1, Ordering::SeqCst);
}

unsafe extern "system" fn original(_: *mut c_void, frame: *mut c_void, result: *mut c_void) {
    assert!(result.is_null(), "Void original receives unchanged result pointer");
    let mut string = FString("entry-string".into());
    let mut text = FText(Arc::new("entry-text".into()));
    unsafe {
        unchained_macros_kismet::observation::observe_step(frame, (&mut string as *mut FString).cast(), 11, || {});
        unchained_macros_kismet::observation::observe_step(frame, (&mut text as *mut FText).cast(), 12, || {});
    }

    string.0.clear();
    ORIGINALS.fetch_add(1, Ordering::SeqCst);
    // The original wrapper destroys both VM temporaries.
}

struct Fixture;
unsafe impl Reflection for Fixture {
    unsafe fn find_function(&self, _: &str, _: &str) -> Result<FunctionMetadata, String> {
        Ok(FunctionMetadata {
            is_native: true,
            exec_thunk: original as *const () as usize,
            num_parms: 2,
            return_value_offset: u16::MAX,
        })
    }

    unsafe fn find_exec_schema(&self, _: &str, _: &str) -> Result<FunctionSchema, String> {
        Ok(FunctionSchema {
            function_identity: 42,
            unique_exec_thunk: true,
            parameters: vec![
                PropertyMetadata {
                    identity: 11,
                    kind: PropertyKind::String,
                    size: size_of::<FString>(),
                    array_dim: 1,
                    flags: PARM | CONST_PARM,
                    offset: 0,
                },

                PropertyMetadata {
                    identity: 12,
                    kind: PropertyKind::Text,
                    size: size_of::<FText>(),
                    array_dim: 1,
                    flags: PARM | CONST_PARM,
                    offset: size_of::<FString>(),
                },
            ],
        })
    }
}

#[test]
fn entry_borrows_do_not_steal_retain_or_double_drop_vm_owners() {
    let context = unsafe {
        KismetContext::new(
            Fixture,
            ExecEnvironment {
                module_base: 1,
                frame_step: 2,
                frame_step_explicit: 3,
                frame_code_offset: 0,
            },
        )
    };

    let address = unsafe {
        Library::__UNCHAINED_EXEC_Library_Owners_RESOLVER
            .implementation
            .resolve(&context)
    }
    .unwrap();
    unsafe { Library::__UNCHAINED_EXEC_Library_Owners_BINDING.initialize_address(address) }.unwrap();
    unsafe {
        Library::__UNCHAINED_TARGET_Owners
            .entry
            .register(owners_entry, true, || true)
    }
    .unwrap();
    unsafe {
        (Library::__UNCHAINED_EXEC_Library_Owners_dispatch)(
            std::ptr::null_mut(),
            std::ptr::dangling_mut(),
            std::ptr::null_mut(),
        );
    }

    assert_eq!(ORIGINALS.load(Ordering::SeqCst), 1);
    assert_eq!(ENTRIES.load(Ordering::SeqCst), 1);
    assert_eq!(STRING_DROPS.load(Ordering::SeqCst), 1);
    assert_eq!(TEXT_DROPS.load(Ordering::SeqCst), 1);
}
