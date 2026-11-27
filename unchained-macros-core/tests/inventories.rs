//! Trait-object discovery keeps concrete backend identity without duplicate indexes.
use unchained_macros_core as bindings;
use bindings::__private::{FunctionInfo, PatchInfo, ResolverInfo};

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
            type Target = bindings::NativeTarget<fn(), unsafe fn(&u32), (), (), Backend>;
            static TARGET: Target = unsafe { Target::new(&BINDING, || {}, |_| panic!("discovery must not attach")) };
            pub static HOOK: bindings::DetourRegistration<Target> = bindings::DetourRegistration::new(
                "SameHook", &TARGET, || {}, || panic!("discovery must not evaluate conditions"),
            );
            pub static LISTENER: bindings::ListenerRegistration = bindings::ListenerRegistration::entry(
                "SameListener", &(&TARGET, |_| {}),
                || panic!("discovery must not evaluate conditions"),
            );
            pub static PATCH: bindings::PatchRegistration<Backend> = bindings::PatchRegistration {
                name: stringify!($module), resolver: &RESOLVER,
                factory: |_| panic!("discovery must not run factories"),
                selector: None, applies_to: |_| true, validate_group: |_| Ok(()),
            };
            bindings::__private::inventory::submit! { &RESOLVER as &dyn ResolverInfo }
            bindings::__private::inventory::submit! { &BINDING as &dyn FunctionInfo }
            bindings::__private::inventory::submit! { &HOOK as &dyn bindings::DetourInfo }
            bindings::__private::inventory::submit! { &LISTENER }
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
            assert!(std::ptr::eq(functions[0], &$module::BINDING));
            let hooks = bindings::detour_inventory_for::<$module::Backend>().collect::<Vec<_>>();
            assert_eq!(hooks.len(), 1);
            assert!(std::ptr::addr_eq(hooks[0], &$module::HOOK as &dyn bindings::DetourInfo));
            assert!(std::ptr::eq($module::HOOK.target().binding(), &$module::BINDING));
            assert_eq!(hooks[0].resolver().identity(),
                resolvers[0] as *const bindings::Resolver<$module::Backend> as *const ());
            let listeners = bindings::listener_inventory_for::<$module::Backend>().collect::<Vec<_>>();
            assert_eq!(listeners.len(), 1);
            assert!(std::ptr::eq(listeners[0], &$module::LISTENER));
            let patches = bindings::patch_inventory_for::<$module::Backend>().collect::<Vec<_>>();
            assert_eq!(patches.len(), 1);
            assert!(std::ptr::eq(patches[0], &$module::PATCH));
        };
    }
    check!(first);
    check!(second);
    assert_ne!(first::LISTENER.slot_identity(), second::LISTENER.slot_identity());
}

#[test]
fn erased_queries_preserve_concrete_identity_and_do_not_invoke_native_work() {
    assert_eq!(bindings::__private::inventory::iter::<&dyn ResolverInfo>.into_iter().count(), 2);
    assert_eq!(bindings::__private::inventory::iter::<&dyn FunctionInfo>.into_iter().count(), 2);
    assert_eq!(bindings::__private::inventory::iter::<&dyn bindings::DetourInfo>.into_iter().count(), 2);
    assert_eq!(bindings::__private::inventory::iter::<&bindings::ListenerRegistration>.into_iter().count(), 2);
    assert_eq!(bindings::__private::inventory::iter::<&dyn PatchInfo>.into_iter().count(), 2);
    assert!(!bindings::detour_is_initialized("SameHook"));
    assert!(!bindings::detour_is_enabled("SameHook"));
    bindings::validate_patch_registrations().unwrap();
    bindings::SelectorSnapshot::default().validate_registered().unwrap();
}
