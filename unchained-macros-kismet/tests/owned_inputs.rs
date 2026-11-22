//! Host snapshot extensions must outlive original VM temporary destruction.
use std::{ffi::c_void, mem::size_of, sync::{Arc, atomic::{AtomicUsize, Ordering}}};
use unchained_macros_core::{AddressResolver, ObserverSnapshot};
use unchained_macros_kismet::{kismet_exec_bindings, FunctionMetadata, Reflection, KismetContext, ExecEnvironment};
use unchained_macros_kismet::schema::*;

static STRING_DROPS: AtomicUsize = AtomicUsize::new(0);
static TEXT_DROPS: AtomicUsize = AtomicUsize::new(0);
static ORIGINALS: AtomicUsize = AtomicUsize::new(0);
static EXITS: AtomicUsize = AtomicUsize::new(0);
struct FString(String);
impl Drop for FString { fn drop(&mut self) { STRING_DROPS.fetch_add(1, Ordering::SeqCst); } }
struct FText(Arc<String>);
impl Drop for FText { fn drop(&mut self) { TEXT_DROPS.fetch_add(1, Ordering::SeqCst); } }
// These are host-independent ownership fixtures, not game FText/FString cloning.
unsafe impl ObserverSnapshot for FString {
    type Snapshot = Self;
    fn snapshot(&self) -> Self { Self(self.0.clone()) }
    fn borrow(snapshot: &Self) -> &Self { snapshot }
}
unsafe impl ObserverSnapshot for FText {
    type Snapshot = Self;
    fn snapshot(&self) -> Self { Self(self.0.clone()) }
    fn borrow(snapshot: &Self) -> &Self { snapshot }
}
struct Library;
#[kismet_exec_bindings("/Script/Fixture.Inputs")]
impl Library {
    #[exec_observe(verified, dummied)]
    unsafe fn Owners(string: &FString, text: &FText) {}
}
#[unchained_macros_core::bind_func_exit_listener(Library::Owners)]
unsafe fn owners_exit(string: &FString, text: &FText, _result: &()) {
    assert_eq!(string.0, "entry-string", "Input snapshot precedes native mutation");
    assert_eq!(*text.0, "entry-text");
    assert_eq!(Arc::strong_count(&text.0), 1, "Only snapshot ownership survives");
    assert_eq!(STRING_DROPS.load(Ordering::SeqCst), 1);
    assert_eq!(TEXT_DROPS.load(Ordering::SeqCst), 1);
    EXITS.fetch_add(1, Ordering::SeqCst);
}
unsafe extern "system" fn original(_: *mut c_void, frame: *mut c_void, result: *mut c_void) {
    assert!(result.is_null(), "Void original receives unchanged result pointer");
    let mut string = FString("entry-string".into());
    let mut text = FText(Arc::new("entry-text".into()));
    unsafe {
        unchained_macros_kismet::observation::observe_step(frame, (&mut string as *mut FString).cast(), 11, || {});
        unchained_macros_kismet::observation::observe_step(frame, (&mut text as *mut FText).cast(), 12, || {});
    }
    string.0.clear(); // The snapshot must remain independent.
    ORIGINALS.fetch_add(1, Ordering::SeqCst);
    // Both VM temporaries die before the exit observer.
}
struct Fixture;
unsafe impl Reflection for Fixture {
    unsafe fn find_function(&self, _: &str, _: &str) -> Result<FunctionMetadata, String> {
        Ok(FunctionMetadata { is_native: true, exec_thunk: original as *const () as usize,
            num_parms: 2, return_value_offset: u16::MAX })
    }
    unsafe fn find_exec_schema(&self, _: &str, _: &str) -> Result<FunctionSchema, String> {
        Ok(FunctionSchema { function_identity: 42, unique_exec_thunk: true, parameters: vec![
            PropertyMetadata { identity: 11, kind: PropertyKind::String, size: size_of::<FString>(), array_dim: 1, flags: PARM | CONST_PARM, offset: 0 },
            PropertyMetadata { identity: 12, kind: PropertyKind::Text, size: size_of::<FText>(), array_dim: 1, flags: PARM | CONST_PARM, offset: size_of::<FString>() },
        ] })
    }
}

#[test]
fn owned_host_snapshots_survive_original_cleanup_without_stealing_or_double_drop() {
    let context = unsafe { KismetContext::new(Fixture, ExecEnvironment {
        module_base: 1, frame_step: 2, frame_step_explicit: 3, frame_code_offset: 0,
    }) };
    let address = unsafe { __UNCHAINED_EXEC_Library_Owners_RESOLVER.implementation.resolve(&context) }.unwrap();
    unsafe { __UNCHAINED_EXEC_Library_Owners_BINDING.initialize_address(address) }.unwrap();
    unsafe { Library::__UNCHAINED_LISTENERS_Owners.exit.register(__unchained_prepare_owners_exit, true, || true) }.unwrap();
    unsafe { __UNCHAINED_EXEC_Library_Owners_dispatch(std::ptr::null_mut(), std::ptr::dangling_mut(), std::ptr::null_mut()); }
    assert_eq!(ORIGINALS.load(Ordering::SeqCst), 1);
    assert_eq!(EXITS.load(Ordering::SeqCst), 1);
    assert_eq!(STRING_DROPS.load(Ordering::SeqCst), 2);
    assert_eq!(TEXT_DROPS.load(Ordering::SeqCst), 2);
}
