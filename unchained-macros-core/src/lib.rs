#![doc = include_str!("../README.md")]
pub use unchained_macros_core_impl::{runtime_bindings, bind_resolved_func, bind_resolved_value,
    bind_func_detour, register_patch, resolver_selection_key};

mod binding;
mod detour;
mod patch;
mod registry;
mod resolver;
mod selection;

pub use binding::{ResolvedAddress, FunctionBinding, BindingRegistration, FunctionBindingRegistration,
    NativeFunction, NativeSignature, NativeGlobalPointer, NativeGlobalTarget, ValueBinding,
    ValueBindingRegistration, initialize_value_bindings, initialize_value_bindings_where,
    is_required_value, function_is_bound_for, require_function_bindings_for,
    binding_inventory_for, initialize_bindings_for};
pub use detour::{detour_is_enabled, detour_is_initialized};
pub use patch::{Patch, PatchWriter, WriteValue, PatchState, PatchApplication,
    validate_patch_registrations, apply_patches_for};
pub use registry::{CreateHookFn, HookRegistration, PatchRegistration, hook_inventory_for, patch_inventory_for};
pub use resolver::{AddressResolver, Resolver, resolver_inventory_for};
pub use selection::{SelectionKey, ResolverSelector, SelectorSnapshot, SelectorIdentity,
    ResolverDeclaration, ResolverSelection, ResolverFilter};

#[doc(hidden)]
pub mod __private {
    pub use inventory;
    pub use log;
    pub use paste;
    pub use crate::binding::{ErasedFunctionBindingRegistration, publish_function_detours};
    pub use crate::detour::claim_detour_address;
    pub use crate::registry::{ErasedHookRegistration, ErasedPatchRegistration, PatchInfo};
    pub use crate::resolver::ErasedResolverRegistration;
    pub use crate::selection::{SelectorProvider, selector_type, selection_key,
        selection_matches, dictionary_allows, dictionary_lookup, resolver_selections};
    pub use crate::patch::validate_patch_coverage;
}
