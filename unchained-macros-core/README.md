# unchained-macros-core

Backend-independent native methods, detours, patches, and selection filters.
No memory scanner, reflection engine, resolver-kind enum, or startup-phase policy.

`runtime_bindings` attaches native-source documentation to every method in its
annotated impl, including unbound and private methods. Unbound methods retain
their Rust bodies and have no resolver metadata. The Kismet backend uses the
same documentation path.

Implement `AddressResolver` with an associated `Context`. A `Resolver<R>` holds
the implementation, stable name, and optional selector filter.
`ResolverDeclaration` associates generated declarations with their backend type.

- `FunctionBinding<R>` references `Resolver<R>` and stores the native target name.
- `NativeFunction<F, R>` preserves the native signature independently of the backend.
- `HookRegistration<R>` references its binding and names the Rust detour function.
- `PatchRegistration<R>` references its resolver and names the patch factory/static.

Discover typed registrations:

```rust,ignore
resolver_inventory_for::<Backend>()
binding_inventory_for::<Backend>()
hook_inventory_for::<Backend>()
patch_inventory_for::<Backend>()
```

Inventory stores erased indexes internally; these functions return typed references.
Selection and required patch coverage do not combine unrelated backend types.
Patch IDs remain globally unique.

## Initialization

The host chooses when each backend's context is valid:

```rust,ignore
unsafe {
    initialize_bindings_for::<Backend>(&context, &selectors)?;
    apply_patches_for::<Backend, _>(&context, &selectors, &mut writer, &mut patch_state)?;
}
for hook in hook_inventory_for::<Backend>() {
    // Check selection and attachment state, then attach using the host's policy.
}
```

Keep native callers quiescent during publication and attachment. A retained
`PatchState` prevents completed patches from running again during retries.
Backends decide whether/how to cache successful resolution; core publishes native
bindings once and rejects rebinding.

Address claims and trampoline publication are **global across backends**.
Already-bound aliases receive a newly attached trampoline; aliases bound later
inherit it during initialization. With the required quiescent callers,
original-call bypass does not depend on backend discovery order.

`function_is_bound_for::<Backend>(name)` and
`selectors.supports_resolver_for::<Backend>(name)` scope name queries to a backend.

## Native globals

Use `bind_resolved_value(Resolver, Address)` for inline storage, or `Value` for a
pointer slot. Static bindings expose `NativeGlobal<T>::read()` returning a raw
pointer. Read explicitly at each use; dispatch methods through rust-ctables or
the native type's ordinary methods. No target opt-in or generated forwarding is needed.
Initialize registered values with `initialize_value_bindings_where`, filtered by the frozen snapshot.
Keep native spelling, synchronization, and declaring-header placement; never
cache the pointee of a replaceable slot.
