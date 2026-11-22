//! Native binding, detour, patch, and resolver support.
pub use unchained_macros_core::{bind_resolved_func, bind_resolved_value, resolver_selection_key};
pub use unchained_macros_patternsleuth::{runtime_bindings, bind_resolved_request_function};

#[doc = include_str!("../resolver-declarations.md")]
pub use unchained_macros_patternsleuth::define_pattern_resolver;

#[doc = include_str!("../patches.md")]
pub use unchained_macros_core::register_patch;

/// Attach a replacement with the native binding's exact signature.
///
/// `#[mutability_override]` permits either outer pointer/reference mutability on
/// that parameter, including an exact match. Types are checked against the
/// binding. Pointees (including nested mutability), lifetimes,
/// unannotated parameters, arity, and return types remain strict.
/// Spell the outer pointer/reference directly in both signatures, not via aliases.
///
/// # Safety
///
/// Every intercepted argument must satisfy the replacement's validity, aliasing,
/// and write-access contract. In particular, an `&mut T` replacement requires
/// exclusive access for its entire borrow even if the native binding uses `&T`.
/// The annotation does not make shared or read-only memory writable. Raw pointers
/// and references cannot be interchanged, and reference lifetimes cannot change.
///
/// Unannotated pointer mutability must match:
///
/// ```compile_fail
/// use unchained_macros::{runtime_bindings, bind_func_detour};
/// # unchained_macros::define_pattern_resolver!(Resolver, ["AA"]);
/// struct Native;
/// #[runtime_bindings]
/// impl Native {
///     #[bind_resolved_func(Resolver)]
///     unsafe fn read(value: *const u32) {}
/// }
/// #[bind_func_detour(Native::read)]
/// unsafe fn replacement(value: *mut u32) {}
/// ```
///
/// The annotation cannot change the pointee type:
///
/// ```compile_fail
/// use unchained_macros::{runtime_bindings, bind_func_detour};
/// # unchained_macros::define_pattern_resolver!(Resolver, ["AA"]);
/// struct Native;
/// #[runtime_bindings]
/// impl Native {
///     #[bind_resolved_func(Resolver)]
///     unsafe fn read(value: *const u32) {}
/// }
/// #[bind_func_detour(Native::read)]
/// unsafe fn replacement(#[mutability_override] value: *mut u64) {}
/// ```
///
/// Other parameters cannot borrow the override:
///
/// ```compile_fail
/// use unchained_macros::{runtime_bindings, bind_func_detour};
/// # unchained_macros::define_pattern_resolver!(Resolver, ["AA"]);
/// struct Native;
/// #[runtime_bindings]
/// impl Native {
///     #[bind_resolved_func(Resolver)]
///     unsafe fn read(value: *const u32, other: *const u32) {}
/// }
/// #[bind_func_detour(Native::read)]
/// unsafe fn replacement(#[mutability_override] value: *mut u32, other: *mut u32) {}
/// ```
///
/// Nested pointer mutability is part of the pointee type:
///
/// ```compile_fail
/// use unchained_macros::{runtime_bindings, bind_func_detour};
/// # unchained_macros::define_pattern_resolver!(Resolver, ["AA"]);
/// struct Native;
/// #[runtime_bindings]
/// impl Native {
///     #[bind_resolved_func(Resolver)]
///     unsafe fn read(value: *const *const u32) {}
/// }
/// #[bind_func_detour(Native::read)]
/// unsafe fn replacement(#[mutability_override] value: *mut *mut u32) {}
/// ```
///
/// Arity and returns remain exact:
///
/// ```compile_fail
/// use unchained_macros::{runtime_bindings, bind_func_detour};
/// # unchained_macros::define_pattern_resolver!(Resolver, ["AA"]);
/// struct Native;
/// #[runtime_bindings]
/// impl Native {
///     #[bind_resolved_func(Resolver)]
///     unsafe fn read(value: *const u32, count: u32) {}
/// }
/// #[bind_func_detour(Native::read)]
/// unsafe fn replacement(#[mutability_override] value: *mut u32) {}
/// ```
///
/// ```compile_fail
/// use unchained_macros::{runtime_bindings, bind_func_detour};
/// # unchained_macros::define_pattern_resolver!(Resolver, ["AA"]);
/// struct Native;
/// #[runtime_bindings]
/// impl Native {
///     #[bind_resolved_func(Resolver)]
///     unsafe fn read(value: *const u32) -> u32 {}
/// }
/// #[bind_func_detour(Native::read)]
/// unsafe fn replacement(#[mutability_override] value: *mut u32) -> u64 { 0 }
/// ```
///
/// An override cannot narrow lifetimes, even inside a raw pointee:
///
/// ```compile_fail
/// use unchained_macros::{runtime_bindings, bind_func_detour};
/// # unchained_macros::define_pattern_resolver!(Resolver, ["AA"]);
/// struct Native;
/// #[runtime_bindings]
/// impl Native {
///     #[bind_resolved_func(Resolver)]
///     unsafe fn read<'a>(value: *const &'a u32) {}
/// }
/// #[bind_func_detour(Native::read)]
/// unsafe fn replacement(#[mutability_override] value: *mut &'static u32) {}
/// ```
///
/// ```compile_fail
/// use unchained_macros::{runtime_bindings, bind_func_detour};
/// # unchained_macros::define_pattern_resolver!(Resolver, ["AA"]);
/// struct Native;
/// #[runtime_bindings]
/// impl Native {
///     #[bind_resolved_func(Resolver)]
///     unsafe fn read<'a>(value: *const u32, other: &'a u32) -> &'a u32 {}
/// }
/// #[bind_func_detour(Native::read)]
/// unsafe fn replacement(#[mutability_override] value: *mut u32, other: &'static u32)
///     -> &'static u32 { other }
/// ```
///
/// Raw pointers and references cannot be interchanged:
///
/// ```compile_fail
/// use unchained_macros::{runtime_bindings, bind_func_detour};
/// # unchained_macros::define_pattern_resolver!(Resolver, ["AA"]);
/// struct Native;
/// #[runtime_bindings]
/// impl Native {
///     #[bind_resolved_func(Resolver)]
///     unsafe fn read(value: &u32) {}
/// }
/// #[bind_func_detour(Native::read)]
/// unsafe fn replacement(#[mutability_override] value: *mut u32) {}
/// ```
///
/// Reference lifetimes and nested mutability remain strict:
///
/// ```compile_fail
/// use unchained_macros::{runtime_bindings, bind_func_detour};
/// # unchained_macros::define_pattern_resolver!(Resolver, ["AA"]);
/// struct Native;
/// #[runtime_bindings]
/// impl Native {
///     #[bind_resolved_func(Resolver)]
///     unsafe fn read<'a>(value: &'a u32) -> &'a u32 {}
/// }
/// #[bind_func_detour(Native::read)]
/// unsafe fn replacement(#[mutability_override] value: &'static mut u32)
///     -> &'static u32 { value }
/// ```
///
/// ```compile_fail
/// use unchained_macros::{runtime_bindings, bind_func_detour};
/// # unchained_macros::define_pattern_resolver!(Resolver, ["AA"]);
/// struct Native;
/// #[runtime_bindings]
/// impl Native {
///     #[bind_resolved_func(Resolver)]
///     unsafe fn read(value: &&mut u32) {}
/// }
/// #[bind_func_detour(Native::read)]
/// unsafe fn replacement(#[mutability_override] value: &mut &u32) {}
/// ```
///
/// Unannotated references keep their own mutability:
///
/// ```compile_fail
/// use unchained_macros::{runtime_bindings, bind_func_detour};
/// # unchained_macros::define_pattern_resolver!(Resolver, ["AA"]);
/// struct Native;
/// #[runtime_bindings]
/// impl Native {
///     #[bind_resolved_func(Resolver)]
///     unsafe fn read(value: &u32, other: &u32) {}
/// }
/// #[bind_func_detour(Native::read)]
/// unsafe fn replacement(#[mutability_override] value: &mut u32, other: &mut u32) {}
/// ```
pub use unchained_macros_core::*;
pub use unchained_macros_patternsleuth::{define_custom_resolver, require_resolver,
    PatternsleuthResolver, ScanContext, SelectorSnapshotExt, RequiredResolver, OffsetRegisty,
    startup_resolvers, initialize_function_bindings, initialize_function_bindings_where,
    apply_registered_patches, __pattern_resolver};
pub type ResolverDescriptor<R = PatternsleuthResolver> = unchained_macros_core::Resolver<R>;
pub type FunctionBinding<R = PatternsleuthResolver> = unchained_macros_core::FunctionBinding<R>;
pub type FunctionBindingRegistration<R = PatternsleuthResolver> = unchained_macros_core::FunctionBindingRegistration<R>;
pub type HookRegistration<R = PatternsleuthResolver> = unchained_macros_core::HookRegistration<R>;
pub type PatchRegistration<R = PatternsleuthResolver> = unchained_macros_core::PatchRegistration<R>;
pub type NativeFunction<F, R = PatternsleuthResolver> = unchained_macros_core::NativeFunction<F, R>;

#[doc(hidden)]
pub mod __private {
    pub use unchained_macros_core::__private::*;
    pub use unchained_macros_patternsleuth::__private::{context_snapshot, single_address};
}
