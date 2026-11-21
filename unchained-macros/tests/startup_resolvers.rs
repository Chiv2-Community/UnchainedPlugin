use binary_bloodhound::BinaryMatcher;
use object::SectionKind;
use patternsleuth::{
    Memory, NamedMemorySection,
    image::{Image, ImageType, pe::PEImage},
};
use std::sync::atomic::{AtomicUsize, Ordering};
use unchained_macros::{bind_resolved_func, bind_resolved_value, define_custom_resolver, define_pattern_resolver};

static DEPENDENCIES: AtomicUsize = AtomicUsize::new(0);
static SIDE_EFFECTS: AtomicUsize = AtomicUsize::new(0);

define_custom_resolver!(Dependency, BinaryMatcher::from_sync(|()| {
    DEPENDENCIES.fetch_add(1, Ordering::SeqCst);
    Ok(vec![0x1000])
}).ensure_one());

define_pattern_resolver!(BoundFixture, ["AA BB"], |ctx, _patterns| {
    Ok(ctx.resolve(Dependency::resolver()).await?.0)
});

define_custom_resolver!(UnusedFixture, BinaryMatcher::from_sync(|()| {
    panic!("unconsumed declarations must not run")
}).ensure_one());

define_pattern_resolver!(SideEffectFixture, ["AA BB"], |ctx, _patterns| {
    SIDE_EFFECTS.fetch_add(1, Ordering::SeqCst);
    Ok(ctx.resolve(Dependency::resolver()).await?.0 + 0x1000)
});
unchained_macros::require_resolver!(SideEffectFixture);

define_pattern_resolver!(ValueFixture, ["AA BB"]);
define_pattern_resolver!(PatchFixture, ["AA BB"]);
define_pattern_resolver!(MethodResolver, ["AA BB"]);

struct MethodFixture;

#[unchained_macros::runtime_bindings]
impl MethodFixture {
    #[bind_resolved_func(MethodResolver)]
    pub unsafe fn call() {}

    #[unchained_macros::bind_resolved_request_function(GetPost_MethodFixture)]
    pub unsafe fn request() {}

    #[cfg(any())]
    #[bind_resolved_func(MissingDisabledMethod)]
    pub unsafe fn disabled() {}
}

#[bind_resolved_func(BoundFixture)]
unsafe fn bound() {}
#[bind_resolved_func(BoundFixture)]
unsafe fn alias() {}
#[bind_resolved_value(ValueFixture, Address)]
unsafe fn value() -> *mut u64 {}

#[unchained_macros::bind_resolved_request_function(GetPost_Fixture)]
unsafe fn request() {}

#[unchained_macros::register_patch(PatchFixture)]
fn fixture_patch(_address: u64) -> unchained_macros::Patch {
    panic!("selection must not construct or apply patches")
}

// Simulate an upstream resolver with the same key as our native declaration.
mod upstream {
    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    pub struct BoundFixture(pub u64);
    patternsleuth::resolvers::impl_resolver_singleton!(all, BoundFixture, |_ctx| async {
        panic!("the upstream namesake must not run")
    });
    #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
    pub struct ExternalFixture(pub u64);
    patternsleuth::resolvers::impl_resolver_singleton!(all, ExternalFixture, |_ctx| async {
        Ok(Self(0x3000))
    });
}

#[bind_resolved_func(upstream::ExternalFixture)]
unsafe fn external() {}

#[cfg(any())]
#[bind_resolved_func(MissingDisabledFixture)]
unsafe fn disabled() {}

#[test]
fn only_consumers_and_explicit_roots_run_with_shared_transitive_dependencies() {
    let roots = unchained_macros::startup_resolvers().unwrap();
    let names: Vec<_> = roots.iter().map(|root| root.name).collect();
    assert_eq!(names, ["BoundFixture", "ExternalFixture", "MethodResolver", "PatchFixture", "SideEffectFixture", "ValueFixture"]);
    let image = Image::from_memory(
        0x1000,
        Memory::from_sections(vec![NamedMemorySection::new(
            ".text".into(), 0x1000, SectionKind::Text, vec![0xAA, 0xBB],
        )]),
        ImageType::PEImage(PEImage {
            exception_directory_range: 0..0,
            exception_children_cache: Default::default(),
        }),
    );
    let results = image.resolve_many(&roots.iter().map(|root| root.getter).collect::<Vec<_>>());
    assert!(results.iter().all(Result::is_ok), "{results:?}");
    assert_eq!(DEPENDENCIES.load(Ordering::SeqCst), 1);
    assert_eq!(SIDE_EFFECTS.load(Ordering::SeqCst), 1);
    // Keep the synthetic binding declarations reachable without invoking native code.
    let _ = (bound, alias, value, request, external, MethodFixture::call, MethodFixture::request);
}
