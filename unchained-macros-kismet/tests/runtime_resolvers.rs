//! Backend isolation, retries, and cross-backend original-call publication.
#![cfg(windows)]

use bindings::__private::inventory;
use bindings::{AddressResolver, Patch, PatchState, PatchWriter, Resolver, SelectorSnapshot};
use std::{
    num::NonZeroUsize,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use unchained_macros_core as bindings;
use unchained_macros_kismet::{ExecEnvironment, FunctionMetadata, KismetContext, KismetReflectionResolver, Reflection};

static READY: AtomicBool = AtomicBool::new(false);
static READS: AtomicUsize = AtomicUsize::new(0);

#[inline(never)]
unsafe extern "C" fn add(value: i32) -> i32 {
    unsafe { std::ptr::read_volatile(&value) }.wrapping_add(1)
}
#[inline(never)]
unsafe extern "C" fn plain(value: i32) -> i32 {
    unsafe { std::ptr::read_volatile(&value) }.wrapping_add(2)
}
// Synthetic thunks are decoded, never executed. Their first call models parameter
// stepping; the second calls the same native body used by the direct bindings.
struct Frame {
    value: i32,
}
#[inline(never)]
unsafe extern "C" fn step(frame: *const Frame) -> i32 {
    unsafe { std::ptr::read_volatile(std::ptr::addr_of!((*frame).value)) }
}
#[inline(never)]
unsafe extern "C" fn explicit_step(frame: *const Frame) -> i32 {
    unsafe { std::ptr::read_volatile(std::ptr::addr_of!((*frame).value)) }.wrapping_add(1)
}
#[inline(never)]
unsafe extern "C" fn add_thunk(_: *mut (), frame: *const Frame, result: *mut i32) {
    let value = unsafe { step(frame) };
    let value = unsafe { add(value) };
    unsafe {
        result.write(value);
    }
}
#[inline(never)]
unsafe extern "C" fn plain_thunk(_: *mut (), frame: *const Frame, result: *mut i32) {
    let value = unsafe { step(frame) };
    let value = unsafe { plain(value) };
    unsafe {
        result.write(value);
    }
}

struct FixtureReflection;
// SAFETY: Metadata identifies the synthetic functions and their in-module callees.
unsafe impl Reflection for FixtureReflection {
    unsafe fn find_function(&self, owner: &str, member: &str) -> Result<FunctionMetadata, String> {
        assert_eq!(owner, "/Script/Fixture.Native");
        READS.fetch_add(1, Ordering::SeqCst);
        if !READY.load(Ordering::SeqCst) {
            return Err("not ready".into());
        }
        let exec_thunk = match member {
            "add" | "alias" => add_thunk as *const () as usize,
            "plain" => plain_thunk as *const () as usize,
            _ => panic!("excluded lookup must not run"),
        };
        Ok(FunctionMetadata {
            is_native: true,
            exec_thunk,
            num_parms: 2,
            return_value_offset: 0,
        })
    }
}

fn environment() -> ExecEnvironment {
    let mut image = 0;
    let entry = unsafe {
        windows::Win32::System::Diagnostics::Debug::RtlLookupFunctionEntry(
            add_thunk as *const () as u64,
            &mut image,
            None,
        )
    };
    assert!(!entry.is_null(), "synthetic thunk must have unwind metadata");
    ExecEnvironment {
        module_base: image as usize,
        frame_step: step as *const () as usize,
        frame_step_explicit: explicit_step as *const () as usize,
        frame_code_offset: 0, // Unused by these parameterized thunks.
    }
}

struct Native;
#[unchained_macros_kismet::kismet_runtime_bindings("/Script/Fixture.Native")]
impl Native {
    unsafe fn add(value: i32) -> i32 {}
    unsafe fn alias(value: i32) -> i32 {}
    #[cfg(any())]
    unsafe fn excluded() {}
}
#[bindings::runtime_bindings]
impl Native {
    #[bind_resolved_func(PlainPatch)]
    unsafe fn plain(value: i32) -> i32 {}
}
#[bindings::bind_func_detour(Native::add)]
unsafe fn replacement(value: i32) -> i32 {
    unsafe { Native::add(value) + 10 }
}

#[bindings::resolver_selection_key]
enum Mode {
    Disabled,
}
impl bindings::ResolverSelector for Mode {
    fn current() -> Self {
        Self::Disabled
    }
}
static FILTERED: Resolver<KismetReflectionResolver> = Resolver {
    name: "Filtered",
    implementation: KismetReflectionResolver::new("/Script/Fixture.Native", "filtered"),
    filter: Some(bindings::ResolverFilter {
        selector: std::any::TypeId::of::<Mode>,
        lookup: |key| bindings::__private::dictionary_lookup(key, &[(Mode::Disabled, false)]),
    }),
};
static FILTERED_BINDING: bindings::FunctionBinding<KismetReflectionResolver> =
    bindings::FunctionBinding::new("Filtered::call", &FILTERED);
inventory::submit! {
    &FILTERED_BINDING
        as &dyn bindings::__private::FunctionInfo
}

struct OtherResolver;
impl AddressResolver for OtherResolver {
    type Context = ();
    unsafe fn resolve(&self, _: &()) -> Result<NonZeroUsize, String> {
        Ok(NonZeroUsize::new(add as *const () as usize).unwrap())
    }
}
struct LateResolver;
impl AddressResolver for LateResolver {
    type Context = ();
    unsafe fn resolve(&self, _: &()) -> Result<NonZeroUsize, String> {
        Ok(NonZeroUsize::new(add as *const () as usize).unwrap())
    }
}
static LATE: Resolver<LateResolver> = Resolver {
    name: "Late",
    implementation: LateResolver,
    filter: None,
};
static LATE_BINDING: bindings::FunctionBinding<LateResolver> = bindings::FunctionBinding::new("Late::alias", &LATE);
inventory::submit! {
    &LATE_BINDING
        as &dyn bindings::__private::FunctionInfo
}
static OTHER: Resolver<OtherResolver> = Resolver {
    name: "Native__add",
    implementation: OtherResolver,
    filter: None,
};
static OTHER_BINDING: bindings::FunctionBinding<OtherResolver> = bindings::FunctionBinding::new("Other::alias", &OTHER);
inventory::submit! {
    &OTHER_BINDING
        as &dyn bindings::__private::FunctionInfo
}
inventory::submit! { &OTHER as &dyn bindings::__private::ResolverInfo }

struct SuppressedResolver;
impl AddressResolver for SuppressedResolver {
    type Context = ();
    unsafe fn resolve(&self, _: &()) -> Result<NonZeroUsize, String> {
        panic!("excluded backend must not run")
    }
}
static SUPPRESSED: Resolver<SuppressedResolver> = Resolver {
    name: "Native__add",
    implementation: SuppressedResolver,
    filter: Some(bindings::ResolverFilter {
        selector: std::any::TypeId::of::<Mode>,
        lookup: |key| bindings::__private::dictionary_lookup(key, &[(Mode::Disabled, false)]),
    }),
};
inventory::submit! { &SUPPRESSED as &dyn bindings::__private::ResolverInfo }

// The patch and native binding share one explicit descriptor and resolver cache.
static PLAIN: Resolver<KismetReflectionResolver> = Resolver {
    name: "Native__plain",
    implementation: KismetReflectionResolver::new("/Script/Fixture.Native", "plain"),
    filter: None,
};
inventory::submit! { &PLAIN as &dyn bindings::__private::ResolverInfo }
struct PlainPatch;
impl bindings::ResolverDeclaration for PlainPatch {
    type Backend = KismetReflectionResolver;
    const NAME: &'static str = "Native__plain";
    const UNRESTRICTED: bool = true;
    const DESCRIPTOR: &'static Resolver<Self::Backend> = &PLAIN;
}
#[bindings::register_patch(PlainPatch)]
static EDIT: Patch = Patch::nop(0, 1);
#[derive(Default)]
struct Writer(Vec<u64>);
impl PatchWriter for Writer {
    unsafe fn bytes(&mut self, address: u64, _: &[u8]) -> Result<(), String> {
        self.0.push(address);
        Ok(())
    }
    unsafe fn nop(&mut self, address: u64, _: usize) -> Result<(), String> {
        self.0.push(address);
        Ok(())
    }
    unsafe fn write(&mut self, address: u64, _: &bindings::WriteValue) -> Result<(), String> {
        self.0.push(address);
        Ok(())
    }
}

#[test]
fn typed_discovery_keeps_backend_lifecycles_separate_but_shares_native_address_ownership() {
    let snapshot = SelectorSnapshot::from_selection(Mode::Disabled).unwrap();
    let context = unsafe { KismetContext::new(FixtureReflection, environment()) };
    assert_eq!(bindings::detour_inventory_for::<OtherResolver>().count(), 0);
    assert_eq!(bindings::patch_inventory_for::<OtherResolver>().count(), 0);
    assert_eq!(bindings::binding_inventory_for::<OtherResolver>().count(), 1);
    assert_eq!(bindings::detour_inventory_for::<KismetReflectionResolver>().count(), 1);
    assert_eq!(
        bindings::resolver_inventory_for::<KismetReflectionResolver>().count(),
        3
    );
    assert!(snapshot.supports_resolver_for::<OtherResolver>("Native__add"));
    assert!(snapshot.supports_resolver_for::<KismetReflectionResolver>("Native__add"));
    assert!(!snapshot.supports_resolver_for::<SuppressedResolver>("Native__add"));
    unsafe {
        bindings::initialize_bindings_for::<OtherResolver>(&(), &snapshot).unwrap();
    }
    assert_eq!(READS.load(Ordering::SeqCst), 0);
    assert!(bindings::function_is_bound_for::<OtherResolver>("Native__add"));
    assert!(!bindings::function_is_bound_for::<KismetReflectionResolver>(
        "Native__add"
    ));
    bindings::require_function_bindings_for::<OtherResolver>(&["Native__add"]).unwrap();
    assert!(bindings::require_function_bindings_for::<KismetReflectionResolver>(&["Native__add"]).is_err());

    let initialize = || unsafe { bindings::initialize_bindings_for::<KismetReflectionResolver>(&context, &snapshot) };
    assert!(initialize().unwrap_err().contains("not ready"));
    assert!(Native::__UNCHAINED_TARGET_add.binding().entry_address().is_none());
    READY.store(true, Ordering::SeqCst);
    initialize().unwrap();
    let reads = READS.load(Ordering::SeqCst);
    initialize().unwrap();
    assert_eq!(READS.load(Ordering::SeqCst), reads);
    assert!(FILTERED_BINDING.entry_address().is_none());
    assert_eq!(unsafe { Native::plain(5) }, 7);

    let mut writer = Writer::default();
    let mut state = PatchState::default();
    for _ in 0..2 {
        unsafe {
            bindings::apply_patches_for::<KismetReflectionResolver, _>(&context, &snapshot, &mut writer, &mut state)
                .unwrap();
        }
    }
    assert_eq!(writer.0, [plain as *const () as u64]);
    assert_eq!(READS.load(Ordering::SeqCst), reads);

    unsafe {
        __UNCHAINED_REGISTRATION_replacement.attach(true).unwrap();
        assert_eq!(std::hint::black_box(add as unsafe extern "C" fn(i32) -> i32)(5), 16);
        assert_eq!(Native::add(5), 6);
        assert_eq!(Native::alias(5), 6);
        assert!(LATE_BINDING.entry_address().is_none());
        bindings::initialize_bindings_for::<LateResolver>(&(), &snapshot).unwrap();
        bindings::initialize_bindings_for::<LateResolver>(&(), &snapshot).unwrap();
        let late: unsafe extern "C" fn(i32) -> i32 = std::mem::transmute(LATE_BINDING.address().unwrap().get());
        assert_eq!(late(5), 6);
        assert_ne!(LATE_BINDING.address(), LATE_BINDING.entry_address());
        let other: unsafe extern "C" fn(i32) -> i32 = std::mem::transmute(OTHER_BINDING.address().unwrap().get());
        assert_eq!(other(5), 6);
        assert_ne!(OTHER_BINDING.address(), OTHER_BINDING.entry_address());
        assert!(bindings::__private::claim_detour_address(OTHER_BINDING.entry_address().unwrap().get()).is_err());
        let hook = bindings::detour_inventory_for::<KismetReflectionResolver>()
            .next()
            .unwrap();
        hook.detour().unwrap().disable().unwrap();
        assert_eq!(Native::add(5), 6);
    }
}
