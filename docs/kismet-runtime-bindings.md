# Reflection-backed bindings

Use Kismet reflection to find a native function by class path and member name
instead of maintaining a byte signature.

## Choose a binding

| Target | Declaration | What you can do |
|---|---|---|
| Native C++ implementation | `#[kismet_runtime_bindings("…")]` | Call the original, attach a detour, add entry/exit listeners |
| Dummied native implementation | `#[kismet_runtime_bindings("…")]` with `#[dummy]` | Add an entry listener to its unique exec wrapper |
| Virtual/interface method | Explicit `rust-ctables` declaration | Dispatch through the object's vtable; resolve a concrete implementation separately to intercept it |
| Function implemented only in Blueprint | Not supported | Requires a separate script invocation/observation mechanism |

Reflection does not infer the native ABI. Inspect the thunk and implementation
before declaring a callable binding. Never use an exec-thunk address as a C++
function address.

## Declare a native binding

Keep native declarations under `unchained-plugin/src/Chivalry2`, following the
[native source layout](engine-sources.md). Use the reflected owner path, not a
short class name.

```rust,ignore
#[unchained_macros_kismet::kismet_runtime_bindings("/Script/Engine.KismetSystemLibrary")]
impl UKismetSystemLibrary {
    /// # Safety
    /// Call on the game thread with a live world-context object.
    pub(crate) unsafe fn IsServer(context: *mut UObject) -> bool {}
}
```

- Methods must be empty, unsafe, non-generic declarations.
- Match the C++ receiver, arguments, return value, ownership, and hidden return
  storage. Static methods have no receiver; instance methods retain theirs.
- Use the exact reflected member name.
- Keep native source links and safety requirements with the declaration.
- The macro creates a typed resolver value with discovery name `Owner__Method`,
  without generating a resolver marker type. It delegates typed wrappers to
  `runtime_bindings`. Do not add `bind_resolved_func` to the same method.

Once initialized, call the binding directly:

```rust,ignore
let is_server = unsafe { UKismetSystemLibrary::IsServer(context) };
```

This calls the original native implementation. After attachment, it uses the
trampoline and bypasses that target's detour and listeners. Calling before
initialization panics.

## Add plugin behavior

Put callbacks under `unchained-plugin/src/unchained/patches`, separate from native declarations.
Target the binding, not its resolver or Rust wrapper address.

```rust,ignore
#[unchained_macros_core::bind_func_entry_listener(UKismetSystemLibrary::IsServer)]
unsafe fn entering_server_check(context: &*mut UObject) {
    log::debug!("IsServer context: {:p}", *context);
}

#[unchained_macros_core::bind_func_exit_listener(UKismetSystemLibrary::IsServer)]
unsafe fn completed_server_check(result: &bool) {
    log::debug!("IsServer returned {result}");
}
```

For behavior changes, use `#[bind_func_detour(Type::Method)]` with the native
signature. Call `Type::Method` inside the detour to invoke the original.

Each native binding supports one callback: detour, entry listener, or exit
listener. Conditions are attachment-time gates; a false condition skips hook creation.
Listeners take immutable call-scoped borrows and return `()`.

Exit listeners receive only the original result.

See [native bindings](native-bindings.md) for signature checks,
activation, reentry, and original-call contracts.

## Observe a dummy method

Shipping builds can reduce native methods to no-ops or cleanup-only bodies.
Their bodies may share an address with unrelated methods. Observe the unique
exec wrapper instead:

```rust,ignore
#[unchained_macros_kismet::kismet_runtime_bindings("/Script/Engine.KismetSystemLibrary")]
impl UKismetSystemLibrary {
    #[dummy]
    pub(crate) unsafe fn PrintWarning(message: &FString) {}
}

#[unchained_macros_core::bind_func_entry_listener(UKismetSystemLibrary::PrintWarning)]
unsafe fn print_warning(message: &FString) {
    if let Ok(message) = message.copy_to_string() {
        log::info!("{message}");
    }
}
```

`#[dummy]` takes no parameters. It declares an entry-only observation target,
not a callable native method. Exit listeners and detours fail compilation.
One entry listener per method is supported.
Native and dummy declarations can share the same annotated impl.

Before declaring a dummy, verify:

- The native body is a no-op or cleanup-only body. Decoder failure is not proof.
- The exec wrapper is unique to the reflected method, including later-loaded
  functions.
- Declared arguments match the wrapper's top-level Step order and layouts.
- Stepped argument slots remain live through the final argument Step.

The original wrapper evaluates arguments, runs its body, and destroys temporaries
exactly once. The listener runs after the final argument Step, before body
execution and cleanup. No-argument listeners run before the wrapper.

Inputs support booleans, bytes, `i32`, `f32`, UObject pointers, and borrowed
`FString`, `FText`, and `FLinearColor`. Mutable ref/out arguments, arrays, and unsupported layouts
are rejected. No argument ownership transfers
to the listener. Original result storage is forwarded untouched.

Coverage includes calls through that exec wrapper, not direct C++ calls.
Foreign-thread calls forward without observation. The existing print integration
is Steam-only; other storefronts are skipped pending verification.

## Initialization and backend integration

The plugin initializes reflection bindings on the game thread at
`FEngineLoop::Init` entry and retries unresolved work at return. Ordinary
contributors adding declarations do not need another startup hook.

For backend work:

- Keep `EngineReflection` under `unchained-plugin/src/unchained/blueprints/reflection.rs`. It copies
  function/property metadata; decoding belongs to `unchained-macros-kismet`.
- Supply a `KismetContext` with that adapter and an `ExecEnvironment` containing
  the module base, original Step entries, and `FFrame::Code` offset.
- Initialize `KismetReflectionResolver` for native bodies and `KismetExecResolver`
  for dummy wrappers. Use typed `*_inventory_for::<Backend>()` discovery.
- Resolve selected aliases before attaching hooks, with callers quiescent.
  Both backends use core's global address ownership and shared trampolines.
- Dummy observation requires both Step adapters to be attached first.
- Keep contexts on the game thread; they are neither Send nor Sync. Successful
  resolutions are cached for the process; failures remain retryable.

## Decoder limits

Native-body recovery supports Windows x64 exec thunks with recognized argument
stepping, or the supported inline frame advancement for no-argument functions.
It uses executable mappings and unwind metadata to bound decoding.

Unsupported indirect/virtual dispatch, tail branches, missing unwind metadata,
inlined bodies, and folded leaf stubs fail closed. There is no fallback to
calling the exec wrapper with a guessed native ABI.

For macro-level examples and compile-time restrictions, see
[`unchained-macros-kismet`](../unchained-macros-kismet/README.md).
