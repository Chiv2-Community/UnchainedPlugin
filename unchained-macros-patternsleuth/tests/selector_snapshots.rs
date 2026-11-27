#[path = "support/image.rs"]
mod fixture;
use unchained_macros_patternsleuth::SelectorSnapshotExt;

use std::sync::{Mutex, atomic::{AtomicBool, AtomicUsize, Ordering}};
use binary_bloodhound::BinaryMatcher;
use unchained_macros_core::{Patch, PatchWriter, ResolverSelector, SelectorSnapshot, register_patch};
use unchained_macros_patternsleuth::{define_custom_resolver, define_pattern_resolver, require_resolver, startup_resolvers};
use fixture::{image, TEXT};

static LOCK: Mutex<()> = Mutex::new(());
static ALPHA: AtomicBool = AtomicBool::new(true);
static BETA: AtomicBool = AtomicBool::new(true);
static CHAIN: AtomicBool = AtomicBool::new(true);
static ALPHA_READS: AtomicUsize = AtomicUsize::new(0);
static BETA_READS: AtomicUsize = AtomicUsize::new(0);

#[unchained_macros_core::resolver_selection_key(require = [First, Second], fallback = Default)]
enum Alpha { First, Second, Default }
impl ResolverSelector for Alpha {
    fn current() -> Self {
        ALPHA_READS.fetch_add(1, Ordering::SeqCst);
        if ALPHA.load(Ordering::SeqCst) { Self::First } else { Self::Second }
    }
}

#[unchained_macros_core::resolver_selection_key]
enum Beta { Missing, Middle, Leaf }
impl ResolverSelector for Beta {
    fn current() -> Self {
        BETA_READS.fetch_add(1, Ordering::SeqCst);
        if BETA.load(Ordering::SeqCst) { Self::Missing } else { Self::Leaf }
    }
    fn fallback(self) -> Option<Self> {
        match self {
            Self::Missing => Some(Self::Middle),
            Self::Middle if CHAIN.load(Ordering::SeqCst) => Some(Self::Leaf),
            _ => None,
        }
    }
}

define_pattern_resolver!(AlphaResolver, {
    Alpha::First: ["AA BB"], Alpha::Second: None, Alpha::Default: ["11 22"],
});
define_pattern_resolver!(BetaResolver, { Beta::Leaf: ["11 22"] });
define_custom_resolver!(BetaValue, using Beta, |key| {
    BinaryMatcher::address(if key == Beta::Missing { TEXT + 0x30 } else { TEXT + 0x40 }).ensure_one()
});
require_resolver!(AlphaResolver);
require_resolver!(BetaResolver);

struct Native;
#[unchained_macros_core::runtime_bindings]
impl Native {
    #[bind_resolved_func(AlphaResolver)]
    unsafe fn alpha() {}
    #[bind_resolved_func(BetaResolver)]
    unsafe fn beta() {}
}
#[unchained_macros_core::bind_func_detour(Native::alpha)]
unsafe fn alpha_detour() { unsafe { Native::alpha(); } }
#[unchained_macros_core::bind_func_detour(Native::beta)]
unsafe fn beta_detour() { unsafe { Native::beta(); } }

#[register_patch(AlphaResolver, Alpha::First)]
static ALPHA_PATCH: Patch = Patch::bytes(0, &[0xAA]);
#[register_patch(BetaResolver, Beta::Leaf)]
static BETA_PATCH: Patch = Patch::nop(0, 1);
#[register_patch(BetaResolver)]
static BETA_UNIVERSAL: Patch = Patch::nop(0, 1);

fn reset() {
    ALPHA.store(true, Ordering::SeqCst);
    BETA.store(true, Ordering::SeqCst);
    CHAIN.store(true, Ordering::SeqCst);
    ALPHA_READS.store(0, Ordering::SeqCst);
    BETA_READS.store(0, Ordering::SeqCst);
}

#[test]
fn independent_selectors_freeze_values_and_fallbacks_once_per_session() {
    let _guard = LOCK.lock().unwrap();
    reset();
    let snapshot = SelectorSnapshot::capture_registered().unwrap();
    let explicit = SelectorSnapshot::from_selection(Alpha::First).unwrap()
        .with_selection(Beta::Missing).unwrap();
    assert_eq!(snapshot.cache_key(), explicit.cache_key());
    assert!(explicit.with_selection(Alpha::Second).unwrap_err().contains("already captured"));
    assert_eq!(ALPHA_READS.load(Ordering::SeqCst), 1);
    assert_eq!(BETA_READS.load(Ordering::SeqCst), 1);
    assert!(snapshot.chain::<Alpha>().unwrap() == [Alpha::First, Alpha::Default]);
    assert!(snapshot.chain::<Beta>().unwrap() == [Beta::Missing, Beta::Middle, Beta::Leaf]);
    ALPHA.store(false, Ordering::SeqCst);
    BETA.store(false, Ordering::SeqCst);
    CHAIN.store(false, Ordering::SeqCst);

    let image = image(&[0x10]);
    let roots = startup_resolvers(&snapshot).unwrap();
    assert!(roots.iter().any(|root| root.name == "AlphaResolver"));
    assert!(roots.iter().any(|root| root.name == "BetaResolver"));
    let results = snapshot.resolve_many(&image, &[AlphaResolver::dyn_resolver, BetaResolver::dyn_resolver]);
    assert!(results.iter().all(Result::is_ok));
    let value = snapshot.eval(&image, |ctx| Box::pin(async {
        ctx.resolve(BetaValue::resolver()).await.unwrap().0
    }));
    assert_eq!(value, TEXT + 0x30);
    assert_eq!(ALPHA_READS.load(Ordering::SeqCst), 1);
    assert_eq!(BETA_READS.load(Ordering::SeqCst), 1);

    let next = SelectorSnapshot::capture_registered().unwrap();
    assert_ne!(snapshot.cache_key(), next.cache_key());
    assert!(!next.supports_resolver_for::<unchained_macros_patternsleuth::PatternsleuthResolver>("AlphaResolver"));
    assert!(next.supports_resolver_for::<unchained_macros_patternsleuth::PatternsleuthResolver>("BetaResolver"));
    assert!(next.get::<Beta>().unwrap() == Beta::Leaf);
    let alpha_hook = unchained_macros_core::detour_inventory_for::<unchained_macros_patternsleuth::PatternsleuthResolver>()
        .find(|hook| hook.name() == "alpha_detour").unwrap();
    let beta_hook = unchained_macros_core::detour_inventory_for::<unchained_macros_patternsleuth::PatternsleuthResolver>()
        .find(|hook| hook.name() == "beta_detour").unwrap();
    let alpha_binding = Native::__UNCHAINED_TARGET_alpha.binding();
    assert!(alpha_hook.resolver().supports(&snapshot));
    assert!(!alpha_hook.resolver().supports(&next));
    assert!(beta_hook.resolver().supports(&snapshot));
    assert!(beta_hook.resolver().supports(&next));
    assert_eq!(alpha_binding.name(), "Native::alpha");
    assert!(std::ptr::eq(alpha_binding.resolver,
        <AlphaResolver as unchained_macros_core::ResolverDeclaration>::DESCRIPTOR));
    assert!(next.chain::<Alpha>().unwrap() == [Alpha::Second, Alpha::Default]);
    assert!(next.chain::<Beta>().unwrap() == [Beta::Leaf]);
    assert_eq!(ALPHA_READS.load(Ordering::SeqCst), 2);
    assert_eq!(BETA_READS.load(Ordering::SeqCst), 2);

    std::thread::scope(|scope| {
        let old = scope.spawn(|| snapshot.eval(&image, |ctx| Box::pin(async {
            ctx.resolve(BetaValue::resolver()).await.unwrap().0
        })));
        let new = scope.spawn(|| next.eval(&image, |ctx| Box::pin(async {
            ctx.resolve(BetaValue::resolver()).await.unwrap().0
        })));
        assert_eq!(old.join().unwrap(), TEXT + 0x30);
        assert_eq!(new.join().unwrap(), TEXT + 0x40);
    });
    assert_eq!(BETA_READS.load(Ordering::SeqCst), 2);
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: () = snapshot.eval(&image, |_| Box::pin(async { panic!("fixture panic"); }));
    }));
    assert!(panicked.is_err());
    // An ordinary fresh context must capture current providers, not a leaked seed.
    assert_eq!(image.resolve(BetaValue::resolver()).unwrap().0, TEXT + 0x40);
    assert_eq!(ALPHA_READS.load(Ordering::SeqCst), 3);
    assert_eq!(BETA_READS.load(Ordering::SeqCst), 3);
}

#[test]
fn fallback_policy_changes_partition_caches_without_changing_current_values() {
    let _guard = LOCK.lock().unwrap();
    reset();
    let before = SelectorSnapshot::capture_registered().unwrap();
    CHAIN.store(false, Ordering::SeqCst);
    let after = SelectorSnapshot::capture_registered().unwrap();
    assert!(before.get::<Beta>().unwrap() == after.get::<Beta>().unwrap());
    assert_ne!(before.cache_key(), after.cache_key());
    assert!(before.supports_resolver_for::<unchained_macros_patternsleuth::PatternsleuthResolver>("BetaResolver"));
    assert!(!after.supports_resolver_for::<unchained_macros_patternsleuth::PatternsleuthResolver>("BetaResolver"));
    assert!(startup_resolvers(&after).err().unwrap().contains("unavailable"));
    let mut writer = Writer::default();
    assert!(unsafe {
        unchained_macros_core::apply_patches_for::<unchained_macros_patternsleuth::PatternsleuthResolver, _>(
            &unchained_macros_patternsleuth::ScanContext::new(1, &Default::default()), &after, &mut writer, &mut Default::default())
    }.is_err());
    assert_eq!(writer.0, 0);
    let image = image(&[]);
    assert!(before.resolve_many(&image, &[BetaResolver::dyn_resolver])[0].is_ok());
    assert!(after.resolve_many(&image, &[BetaResolver::dyn_resolver])[0].is_err());
}

#[derive(Default)]
struct Writer(usize);
impl PatchWriter for Writer {
    unsafe fn bytes(&mut self, _: u64, _: &[u8]) -> Result<(), String> { self.0 += 1; Ok(()) }
    unsafe fn nop(&mut self, _: u64, _: usize) -> Result<(), String> { self.0 += 1; Ok(()) }
    unsafe fn write(&mut self, _: u64, _: &unchained_macros_core::WriteValue) -> Result<(), String> { self.0 += 1; Ok(()) }
}

#[test]
fn missing_provider_is_an_error_not_a_silently_filtered_consumer() {
    let snapshot = SelectorSnapshot::from_selection(Alpha::First).unwrap();
    assert!(startup_resolvers(&snapshot).err().unwrap().contains("missing"));
    let mut writer = Writer::default();
    let error = unsafe {
        unchained_macros_core::apply_patches_for::<unchained_macros_patternsleuth::PatternsleuthResolver, _>(
            &unchained_macros_patternsleuth::ScanContext::new(1, &Default::default()), &snapshot, &mut writer, &mut Default::default())
    }.unwrap_err();
    assert!(error.contains("missing"));
    assert_eq!(writer.0, 0);
}

#[unchained_macros_core::resolver_selection_key]
enum Cycle { First, Second }
impl ResolverSelector for Cycle {
    fn current() -> Self { Self::First }
    fn fallback(self) -> Option<Self> {
        Some(match self { Self::First => Self::Second, Self::Second => Self::First })
    }
}

#[test]
fn fallback_cycles_are_rejected_before_resolution() {
    assert!(SelectorSnapshot::from_selection(Cycle::First).unwrap_err().contains("cycle"));
}
