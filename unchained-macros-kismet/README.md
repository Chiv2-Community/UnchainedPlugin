# unchained-macros-kismet

Kismet native-body bindings layered on `unchained-macros-core`.

Each reflected method becomes a core `bind_resolved_func` binding, which supplies
its native-source documentation, including for private methods. Reflection lookup
details supplement the C++ implementation and existing safety notes; they do not replace them.
The host implements layout-specific UObject access. This crate owns the attribute,
native-function validation, parameter handling, thunk decoding, and address caching.

```rust,ignore
#[unchained_macros_kismet::kismet_runtime_bindings("/Script/Engine.GameplayStatics")]
impl UGameplayStatics {
    pub unsafe fn GetAllActorsOfClass(
        context: *const UObject,
        actor_class: *mut UClass,
        output: *mut TArray<*mut UObject>,
    ) {}
}
```

- Every method must be an empty unsafe native declaration. Static/instance ABIs,
  lifetimes, hidden return storage, and ownership remain the author's responsibility.
- The attribute generates `Owner__Method` runtime resolver descriptors and delegates
  callable wrappers and signature metadata to `unchained-macros-core::runtime_bindings`.
- Existing `bind_func_detour(Type::Method)` attributes work unchanged. A binding
  without a detour is initialized too; no passive detour is needed for direct calls.
- The consumer needs `unchained-macros-core`; generated code accesses its
  implementation dependencies through hidden core re-exports.
  It constructs a `KismetContext` with a `Reflection` implementation and an
  `ExecEnvironment`. The adapter finds a class/member and copies `FunctionMetadata`:
  native status, raw exec address, parameter count, and return-value offset.
  It does not decode thunks or select native bodies.
- Initialize with `initialize_bindings_for::<KismetReflectionResolver>` on the game
  thread while callers are quiescent. The context is not Send or Sync.
  Resolve and publish aliases before attaching this backend's hooks.
- Use `hook_inventory_for::<KismetReflectionResolver>()` and
  `patch_inventory_for::<KismetReflectionResolver>()` for typed discovery.
- Successful native addresses are cached for the process; failures remain retryable.
  These addresses are not scan-cache requirements or persisted UObject pointers.

## Decoder limits

The host supplies the module base, original frame-step entry addresses, and
`FFrame::Code` offset through `ExecEnvironment`. The library validates native
status, null exec pointers, and parameter counts before decoding. A return
parameter is excluded from the argument count. There is no host callback that
can substitute an arbitrary resolved address for this validation.

`native_body(thunk, module_base, frame_step, frame_step_explicit, parameters)` is Windows x64 only.
It uses unwind metadata to bound the thunk and checks executable mappings.
`ExecThunkParameters::Stepped` discovers the first direct call after the last
recognized frame-step call. For functions with no non-return parameters,
`NoArguments { frame_code_offset }` requires the supported inline
`Code += (Code != nullptr)` sequence before selecting the first direct call.
The host supplies the field offset and selects the mode from reflection metadata;
hidden native return storage is not a reflected argument.
This is candidate discovery, not ABI verification: independently inspect each
declaration's thunk and native target before use.

Indirect calls, external/tail branches, invalid instructions, missing unwind
entries, and unsupported parameter handling fail closed. No-argument thunks
also reject helpers or branches before Code advancement, frame-register replacement,
branches before the native call, and contradictory frame-step calls.
Inlined bodies and shipping-folded leaf stubs are unsupported.
There is **no fallback to the exec thunk**, whose ABI is different.
Generalized VM argument marshalling and interface/vtable recovery are not included.

The context owns its adapter, which need not implement Send or Sync. The context
itself remains game-thread-only:

```compile_fail
fn require_send<T: Send>() {}
require_send::<unchained_macros_kismet::KismetContext>();
```

```compile_fail
fn require_sync<T: Sync>() {}
require_sync::<unchained_macros_kismet::KismetContext>();
```

Unsafe declarations are required:

```compile_fail
struct Native;
#[unchained_macros_kismet::kismet_runtime_bindings("/Script/Fixture.Native")]
impl Native {
    fn Call() {}
}
```

See [integration and binary evidence](../docs/kismet-runtime-bindings.md).
