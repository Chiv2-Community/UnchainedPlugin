//! Trait-object discovery keeps concrete backend identity without duplicate indexes.
use unchained_macros_core as bindings;
use bindings::__private::{FunctionInfo, HookInfo, PatchInfo, ResolverInfo};

macro_rules! backend {
    ($module:ident, $context:ty) => {
        mod $module {
            use super::*;
            pub struct Backend;
            impl bindings::AddressResolver for Backend {
                type Context = $context;
                unsafe fn resolve(&self, _: &Self::Context) -> Result<std::num::NonZeroUsize, String> {
                    panic!("discovery must not resolve")
                }
            }
            pub static RESOLVER: bindings::Resolver<Backend> = bindings::Resolver {
                name: "SameResolver", implementation: Backend, filter: None,
            };
            pub static BINDING: bindings::FunctionBinding<Backend> =
                bindings::FunctionBinding::new("SameBinding", &RESOLVER);
            pub static REGISTRATION: bindings::FunctionBindingRegistration<Backend> =
                bindings::FunctionBindingRegistration { binding: &BINDING };
            pub static HOOK: bindings::HookRegistration<Backend> = bindings::HookRegistration {
                name: "SameHook", binding: &BINDING,
                hook_fn: |_| panic!("discovery must not attach"),
                condition: || panic!("discovery must not evaluate conditions"),
                detour: || None,
                enabled: || false, set_enabled: |_| {},
            };
            pub static PATCH: bindings::PatchRegistration<Backend> = bindings::PatchRegistration {
                name: stringify!($module), resolver: &RESOLVER,
                factory: |_| panic!("discovery must not run factories"),
                selector: None, applies_to: |_| true, validate_group: |_| Ok(()),
            };
            bindings::__private::inventory::submit! { &RESOLVER as &dyn ResolverInfo }
            bindings::__private::inventory::submit! { &REGISTRATION as &dyn FunctionInfo }
            bindings::__private::inventory::submit! { &HOOK as &dyn HookInfo }
            bindings::__private::inventory::submit! { &PATCH as &dyn PatchInfo }
        }
    };
}
backend!(first, ());
backend!(second, std::rc::Rc<()>);

#[test]
fn all_inventories_downcast_to_the_original_registration_for_each_backend() {
    macro_rules! check {
        ($module:ident) => {
            let resolvers = bindings::resolver_inventory_for::<$module::Backend>().collect::<Vec<_>>();
            assert_eq!(resolvers.len(), 1);
            assert!(std::ptr::eq(resolvers[0], &$module::RESOLVER));
            let functions = bindings::binding_inventory_for::<$module::Backend>().collect::<Vec<_>>();
            assert_eq!(functions.len(), 1);
            assert!(std::ptr::eq(functions[0], &$module::REGISTRATION));
            let hooks = bindings::hook_inventory_for::<$module::Backend>().collect::<Vec<_>>();
            assert_eq!(hooks.len(), 1);
            assert!(std::ptr::eq(hooks[0], &$module::HOOK));
            let patches = bindings::patch_inventory_for::<$module::Backend>().collect::<Vec<_>>();
            assert_eq!(patches.len(), 1);
            assert!(std::ptr::eq(patches[0], &$module::PATCH));
        };
    }
    check!(first);
    check!(second);
}

#[test]
fn erased_queries_preserve_concrete_identity_and_do_not_invoke_native_work() {
    assert_eq!(bindings::__private::inventory::iter::<&dyn ResolverInfo>.into_iter().count(), 2);
    assert_eq!(bindings::__private::inventory::iter::<&dyn FunctionInfo>.into_iter().count(), 2);
    assert_eq!(bindings::__private::inventory::iter::<&dyn HookInfo>.into_iter().count(), 2);
    assert_eq!(bindings::__private::inventory::iter::<&dyn PatchInfo>.into_iter().count(), 2);
    assert!(!bindings::detour_is_initialized("SameHook"));
    assert!(!bindings::detour_is_enabled("SameHook"));
    bindings::validate_patch_registrations().unwrap();
    bindings::SelectorSnapshot::default().validate_registered().unwrap();
}
