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
- The attribute generates typed resolver values named `Owner__Method` for discovery,
  not per-method resolver types. Callable wrappers and signature metadata use
  `unchained-macros-core::runtime_bindings`.
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
- Use `detour_inventory_for::<KismetReflectionResolver>()` and
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
Virtual/interface native calls are unsupported by this decoder; use a separately
verified rust-ctables declaration, never an inferred C++ ABI.

## Dummy entry listeners

Verified native bodies use core’s `NativeTarget` with replacement, entry, and
exit capabilities. `#[dummy]` generates a Kismet-owned `DummyExecTarget` with
only `SupportsEntry`; exit and detour attributes fail to type-check.

```rust
use unchained_macros_kismet::kismet_runtime_bindings;
use unchained_macros_core::bind_func_entry_listener;
struct Library;
struct FString; // The host supplies the real native layout.
#[kismet_runtime_bindings("/Script/Fixture.Library")]
impl Library {
    pub unsafe fn NativePredicate(value: i32) -> bool {}

    #[dummy]
    pub unsafe fn PrintWarning(message: &FString) {}
}
#[bind_func_entry_listener(Library::PrintWarning)]
unsafe fn warning(message: &FString) {}
```

- `#[dummy]` takes no parameters. Verification is an unsafe declaration contract,
  not automatic disassembly proof. Verify the current binary's argument order,
  exact top-level Step count, and live independent slots through the final Step.
  The wrapper must remain unique,
  including UFunctions loaded later. The adapter rejects currently reflected
  shared wrappers; no `FFrame::node`/callee relationship is assumed.
- Native and dummy methods can share one `#[kismet_runtime_bindings]` impl.
- `#[dummy]` identifies verified no-op **or cleanup-only**
  native bodies. The marker rejects native detours; the exec wrapper, native
  cleanup-only body, frame advancement, and all temporary destruction still run.
  Decoder failure is never dummy evidence.
- Initialize `KismetExecResolver` bindings, then attach its listener inventory
  on the game thread with callers quiescent. The host must first attach both
  Step adapters through core's global address owner and alias/trampoline sharing.
  The supplied environment retains original Step entry addresses for decoding.
  Each resolver retains the context's constructing/resolving thread identity.
  Foreign-thread wrapper calls forward the original exactly once without observing.
  Windows dispatch checks use allocation-free native thread IDs.
- One entry listener per dummy declaration is supported; exit listeners are
  rejected at compile time. Core owns conditions, activation and attachment.
  Callback/condition panics are isolated;
  original execution and cleanup remain unchanged.
- Entry runs at the final original Step completion, before original body/cleanup.
  Noargs entry runs immediately before the original wrapper. Nested same-frame
  expression Steps do not count as arguments; nested exec scopes are independent.
  Reentrant calls invoke observers again; callbacks must terminate recursive paths.
  No recursion guard is provided. No VM state borrow spans native execution
  or callbacks.
- Callback inputs are immutable `&T` observations. FString/FText are borrowed:
  no ownership transfer, shallow-frame replay, zero-initialized text, reference
  stealing, or double evaluation. Scalars, VM bool32 inputs, UObject pointers
  and borrowed LinearColor are supported. Native bool result storage is bool8.
  Mutable ref/out and array parameters and unsupported kinds/layouts fail before
  attachment. Result metadata is validated, but result storage is forwarded
  unchanged without forming a typed borrow. No exit snapshots are created.
- Coverage is calls through the unique exec wrapper, not direct native calls.
  Exec-only markers are not callable native methods and cannot themselves be
  detoured. Ordinary native bindings retain entry/exit listeners and detours.

Dummy methods cannot have exit listeners, even with Copy inputs and a void result:

```compile_fail
struct Library;
#[unchained_macros_kismet::kismet_runtime_bindings("/Script/Fixture.Library")]
impl Library {
    #[dummy]
    pub unsafe fn RemovedBody(value: i32) {}
}
#[unchained_macros_core::bind_func_exit_listener(Library::RemovedBody)]
unsafe fn returned(result: &()) {}
```

Non-void dummy declarations also reject exit listeners:

```compile_fail
struct Library;
#[unchained_macros_kismet::kismet_runtime_bindings("/Script/Fixture.Library")]
impl Library {
    #[dummy]
    pub unsafe fn RemovedPredicate() -> bool {}
}
#[unchained_macros_core::bind_func_exit_listener(Library::RemovedPredicate)]
unsafe fn returned(result: &bool) {}
```

Dummy methods cannot be detoured:

```compile_fail
struct Library;
#[unchained_macros_kismet::kismet_runtime_bindings("/Script/Fixture.Library")]
impl Library {
    #[dummy]
    pub unsafe fn RemovedBody() {}
}
#[unchained_macros_core::bind_func_detour(Library::RemovedBody)]
unsafe fn replacement() {}
```

Dummy markers cannot be called directly:

```compile_fail
struct Library;
#[unchained_macros_kismet::kismet_runtime_bindings("/Script/Fixture.Library")]
impl Library {
    #[dummy]
    pub unsafe fn RemovedBody() {}
}
unsafe { Library::RemovedBody(); }
```

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

See [reflection binding usage and limits](../docs/kismet-runtime-bindings.md).
