# unchained-macros-kismet

Kismet native-body bindings layered on `unchained-macros-core`.

Each reflected method becomes a core `bind_resolved_func` binding, which supplies
its native-source documentation, including for private methods. Reflection lookup
details supplement the C++ implementation and existing safety notes; they do not replace them.
The game supplies UObject reflection; this crate owns the attribute and thunk decoder.

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
  It constructs a `KismetContext` with a game-specific lookup callback that resolves
  `(class_path, method_name)` to the verified native C++ entry.
- Initialize with `initialize_bindings_for::<KismetReflectionResolver>` on the game
  thread while callers are quiescent. The context is not Send or Sync.
  Resolve and publish aliases before attaching this backend's hooks.
- Use `hook_inventory_for::<KismetReflectionResolver>()` and
  `patch_inventory_for::<KismetReflectionResolver>()` for typed discovery.
- Successful callbacks are cached for the process; failures remain retryable.
  These addresses are not scan-cache requirements or persisted UObject pointers.

## Decoder limits

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

Unsafe declarations are required:

```compile_fail
struct Native;
#[unchained_macros_kismet::kismet_runtime_bindings("/Script/Fixture.Native")]
impl Native {
    fn Call() {}
}
```

See [integration and binary evidence](../docs/kismet-runtime-bindings.md).
