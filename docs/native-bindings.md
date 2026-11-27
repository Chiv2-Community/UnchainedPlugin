# Native bindings

`unchained-plugin/src/Chivalry2/` mirrors our best understanding of the game's
C++ source layout, names, and native types. It is not a complete or authoritative
SDK. Keep plugin behavior under `unchained-plugin/src/unchained/`, not in native
declarations. See [source layout](engine-sources.md).

## Native functions: `runtime_bindings`

Use `runtime_bindings` on an existing native type's inherent impl. Each bound
method has an empty unsafe body and a `bind_resolved_func` resolver:

```rust,ignore
#[unchained_macros_core::runtime_bindings]
impl UWorld {
    #[bind_resolved_func(UWorld__InternalGetNetMode)]
    pub unsafe fn InternalGetNetMode(self: *const Self) -> ENetMode {}
}
```

The resolver finds the native address; the declaration specifies its ABI. Match
the verified receiver, parameters, return storage, and lifetimes. Keep source
links and safety requirements with the declaration. `runtime_bindings` on a
struct or enum adds documentation only; it does not determine its memory layout.

Define scan and offset resolvers with
[`unchained-macros-patternsleuth`](../unchained-macros-patternsleuth/resolvers.md).
Virtual methods use `rust-ctables`, not runtime bindings.

Calling a binding invokes the original implementation and bypasses our detour
and listeners on that method. Calling before initialization panics:

```rust,ignore
let mode = unsafe { UWorld::InternalGetNetMode(world) };
```

## Native globals: `bind_resolved_value`

Specify whether the resolver finds a pointer slot or an inline object:

```rust,ignore
#[unchained_macros_core::bind_resolved_value(
    patternsleuth::resolvers::unreal::gmalloc::GMalloc, Value
)]
pub static GMalloc: *mut FMalloc;

#[unchained_macros_core::bind_resolved_value(
    patternsleuth::resolvers::unreal::guobject_array::GUObjectArray, Address
)]
pub static GUObjectArray: *mut FUObjectArray;
```

Read explicitly with `unsafe { GMalloc.read() }` or `unsafe { GUObjectArray.read() }`.
`Value` reloads the current pointer; `Address` returns the inline object's pointer.
Neither transfers ownership or synchronizes access with native writes.

## Reflection: `kismet_runtime_bindings`

Use a reflected class path instead of a scan resolver:

```rust,ignore
#[unchained_macros_kismet::kismet_runtime_bindings("/Script/Engine.KismetSystemLibrary")]
impl UKismetSystemLibrary {
    pub(crate) unsafe fn IsServer(context: *mut UObject) -> bool {}
}
```

Do not add `runtime_bindings` or `bind_resolved_func` to these declarations.
Reflection locates the native body, not its ABI. Startup initializes these
bindings on the game thread once reflection is ready.

For a verified dummied body, use the same macro with parameterless `#[dummy]`:

```rust,ignore
#[unchained_macros_kismet::kismet_runtime_bindings("/Script/Engine.KismetSystemLibrary")]
impl UKismetSystemLibrary {
    #[dummy]
    pub(crate) unsafe fn PrintWarning(message: &FString) {}
}
```

This exposes **entry observation only** through the unique exec wrapper. It
cannot be called, detoured, or given an exit listener. Blueprint-only functions
are unsupported. See [reflection requirements and limits](kismet-runtime-bindings.md).

## Detours

Target the method, not its resolver. Use its native signature, with an explicit
first argument for an instance receiver:

```rust,ignore
#[unchained_macros_core::bind_func_detour(UKismetSystemLibrary::IsServer)]
unsafe fn log_server_check(context: *mut UObject) -> bool {
    let result = unsafe { UKismetSystemLibrary::IsServer(context) };
    log::debug!("IsServer: {result}");
    result
}
```

Call the binding inside the detour to call the original without reentering the
hook. `#[mutability_override]` on a pointer/reference parameter permits an outer
mutability difference, not a different pointee type or lifetime. The callback
must still uphold native validity, aliasing, and write-access requirements.

## Listeners

Entry listeners borrow inputs; exit listeners append a borrowed final result.
Both return `()`:

```rust,ignore
#[unchained_macros_core::bind_func_entry_listener(UKismetSystemLibrary::IsServer)]
unsafe fn entered(context: &*mut UObject) {
    log::debug!("IsServer context: {:p}", *context);
}

#[unchained_macros_core::bind_func_exit_listener(UKismetSystemLibrary::IsServer)]
unsafe fn returned(_context: &*mut UObject, result: &bool) {
    log::debug!("IsServer returned {result}");
}
```

- Exit inputs are entry-time `ObserverSnapshot` snapshots, not post-call out parameters.
  Native reference inputs are currently unsupported; void results use `&()`.
- Borrows are callback-scoped. Pointer observations do not extend pointee lifetimes
  or permit writes. Do not retain borrows or release engine-owned storage.
- Each native method supports one detour, one entry listener, and one exit listener.
  They share a physical hook but have independent activation.
- All callback attributes accept an optional condition, e.g. `Type::Method, || enabled()`.
  It runs at attachment, not on every call. Startup discovers registrations automatically.
- Observer-induced reentry suppresses observers, not native execution. Rust observer
  panics are isolated; exit delivery requires a normally returning native call.
