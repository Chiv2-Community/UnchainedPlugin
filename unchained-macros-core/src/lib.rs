#![doc = include_str!("../README.md")]

pub use unchained_macros_core_impl::{
    bind_func_detour, bind_func_entry_listener, bind_func_exit_listener, bind_resolved_func, bind_resolved_value,
    register_patch, resolver_selection_key, runtime_bindings,
};

mod binding;
mod detour;
mod listener;
mod patch;
mod registry;
mod resolver;
mod selection;
mod target;

pub use target::{NativeTarget, SupportsDetour};

pub use binding::{
    binding_inventory_for, function_is_bound_for, initialize_bindings_for, initialize_value_bindings,
    initialize_value_bindings_where, is_required_value, require_function_bindings_for, FunctionBinding, NativeGlobal,
    NativeSignature, ResolvedAddress, ValueBinding,
};

pub use detour::{detour_is_enabled, detour_is_initialized, NativeHook};
pub use listener::{
    isolate_observer, listener_inventory_for, listener_is_enabled, listener_is_initialized,
    validate_listener_registrations_for, CallbackSlot, CallbackTarget, ExitObservation, ListenerRegistration,
    ObserverSnapshot, SupportsEntry, SupportsExit,
};

pub use patch::{
    apply_patches_for, validate_patch_registrations, Patch, PatchApplication, PatchState, PatchWriter, WriteValue,
};

pub use registry::{detour_inventory_for, patch_inventory_for, DetourInfo, DetourRegistration, PatchRegistration};
pub use resolver::{resolver_inventory_for, AddressResolver, Resolver};
pub use selection::{
    ResolverDeclaration, ResolverFilter, ResolverSelection, ResolverSelector, SelectionKey, SelectorIdentity,
    SelectorSnapshot,
};

#[doc(hidden)]

pub mod __private {

    pub use crate::target::detour_signature;

    pub use crate::binding::FunctionInfo;
    pub use crate::detour::claim_detour_address;
    pub use crate::patch::validate_patch_coverage;
    pub use crate::registry::PatchInfo;
    pub use crate::resolver::ResolverInfo;
    pub use crate::selection::{
        dictionary_allows, dictionary_lookup, resolver_selections, selection_key, selection_matches, selector_type,
        SelectorProvider,
    };

    pub use inventory;
    pub use log;
    pub use paste;
    pub use retour;
}
