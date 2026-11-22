//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Test/Public/Fixture.h>.
#![allow(non_snake_case, dead_code)]

use unchained_macros_core::{bind_resolved_func, runtime_bindings, Resolver, ResolverDeclaration};
use unchained_macros_kismet::{kismet_runtime_bindings, KismetReflectionResolver};

pub mod qualified {
    pub struct Reflected;
}

#[kismet_runtime_bindings("/Script/Fixture.Reflected")]
impl crate::qualified::Reflected {
    /// # Safety
    /// Keep the native receiver alive.
    /// Authored method comment.
    /// Native overload: `()`
    pub unsafe fn Call() -> i32 {}
    pub unsafe fn Typed(value: i32) -> i32 {}
}

macro_rules! named_resolver {
    ($name:ident) => {
        pub struct $name;
        impl ResolverDeclaration for $name {
            type Backend = KismetReflectionResolver;
            const NAME: &'static str = stringify!($name);
            const UNRESTRICTED: bool = true;
            const DESCRIPTOR: &'static Resolver<Self::Backend> = &FREE_DESCRIPTOR;
        }
    };
}
macro_rules! define_pattern_resolver {
    ($name:ident, $pattern:tt) => {
        named_resolver!($name);
    };
}
macro_rules! define_offset_resolver {
    ($name:ident) => {
        named_resolver!($name);
    };
}
define_pattern_resolver!(ScanKey, ["00"]);
define_offset_resolver!(ProviderKey);

pub struct Named;
#[runtime_bindings]
impl Named {
    #[bind_resolved_func(ScanKey)]
    pub unsafe fn Scan() {}
    #[bind_resolved_func(ProviderKey)]
    pub unsafe fn Provided() {}
}

static FREE_DESCRIPTOR: Resolver<KismetReflectionResolver> = Resolver {
    name: "FreeDescriptor",
    implementation: KismetReflectionResolver::new("/Script/Fixture.Free", "Call"),
    filter: None,
};

/// Free-function safety note.
#[bind_resolved_func(KismetReflectionResolver, &FREE_DESCRIPTOR)]
pub unsafe fn free_descriptor() {}
