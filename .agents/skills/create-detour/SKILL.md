---
name: create-detour
description: Create a plugin detour targeting an existing native runtime binding. Use when intercepting a Chivalry 2 function to add logging, publish events, or change behavior under unchained-plugin/src/unchained/patches.
---

# Creating a detour

Attach plugin behavior to a native binding without moving policy into the model.

- Follow [Creating game hooks](../../../CONTRIBUTING.md#creating-game-hooks),
  especially [Create the detour](../../../CONTRIBUTING.md#create-the-detour).
- Read [binding and trampoline contracts](../../../docs/native-bindings.md).

## Verify the binding

Reuse an existing binding unless it is missing or there is evidence it may be wrong.

- Confirm the method path, exact signature, resolver/registry key, feature gates,
  and supported builds.
- Load `create-runtime-binding` only when no binding exists or there is reason
  to suspect the existing binding is incorrect. Use the skill tool when available;
  otherwise read the [runtime binding skill](../create-runtime-binding/SKILL.md).
- In those cases, follow that skill and its applicable resolver prerequisite
  before implementing the detour.
- Do not target a resolver type, wrapper address, or generated vtable method.
  Virtual implementations need a separate resolver-backed binding.
- Access used native globals through matching Rust statics, e.g.
  `unsafe { GMalloc.read().Malloc(size, alignment) }`, not `Globals` accessors or cached
  pointers. Keep their native names/header placement and native synchronization
  obligations. This policy does not turn plugin-owned state into native globals.

## Choose the functional module

Place replacements under `unchained-plugin/src/unchained/patches/*` by plugin purpose.

- Reuse the relevant hosting, backend, networking, message, or other feature module.
- Put hooks that only log in `diagnostics.rs`.
- Do not mirror native classes or header directories in the patches tree.
- Register new modules in their parent `mod.rs`.
- Put imports first, then all detour entry points, then instruction patches,
  helpers, state, types, and tests.
- Gate a whole module when all its contents require the same feature.
- Keep native resolvers and ABI declarations under `unchained-plugin/src/Chivalry2/*`.

## Implement the replacement

Use `bind_func_detour` on an unsafe free function.

- Target `NativeType::Method`; names do not require a `_detour` suffix.
- Use the binding's C++-spelled method name; detour function and parameter names
  use snake_case.
- Native destructor bindings are named `Destructor`; do not simplify their ABI.
- Preserve `TBL` capitalization in native type and method references.
- Match every parameter and return type, including hidden storage and lifetimes.
- Declare an instance receiver explicitly as the first argument.
- Rust checks compatibility with invariant binding metadata; this does not
  establish the native ABI's correctness.
- For intentional outer pointer/reference mutability adaptation, annotate that
  parameter with `#[mutability_override]`. Either direction and exact matches
  are allowed; the native type is inferred. Pointees, nested mutability, other
  parameters, returns, and lifetimes stay exact. Do not interchange raw pointers
  and references. Every native argument must satisfy the detour's validity,
  aliasing, and write-access contract; `&mut T` requires exclusive access for its
  entire borrow, even for native `&T`. The annotation does not authorize writes
  to shared or read-only memory.
- Hooks default to enabled. Add a noncapturing condition only when needed.
- Conditions run before attachment. False skips hook creation.
  Put per-call decisions in the body.
- Only one detour may own a native address; extend an existing replacement
  rather than installing a duplicate.
- Call the native binding for original-call bypass; do not generate a second binding.

Alternative to the existing UDP replacement; do not install both:

```rust
use crate::Chivalry2::Engine::{FString, FUDPPing, FIcmpEchoResultCallback};
use crate::tools::hook_globals::cli_args;

#[unchained_macros_core::bind_func_detour(FUDPPing::UDPEcho, || !cli_args().is_server())]
unsafe fn log_udp_echo(
    endpoint: *const FString,
    timeout: f32,
    callback: *mut FIcmpEchoResultCallback,
) {
    sdebug!("UDP ping requested with timeout {}", timeout);
    unsafe { FUDPPing::UDPEcho(endpoint, timeout, callback) };
}
```

## Preserve native behavior deliberately

Make original-call and lifetime decisions explicit.

- Call the native binding for the original implementation; attached bindings
  use the trampoline even when the detour is disabled.
- Do not call the detour recursively or dispatch through a vtable to reach
  the original; virtual dispatch may re-enter the replacement.
- Preserve return values unless changing them is the requested policy.
- Assume startup initialized required bindings.
- Preserve pointer validity, thread affinity, ownership, and callback lifetimes.
- Do not free engine-owned borrowed buffers or retain pointers beyond validity.
- Do not let Rust panics unwind through native callers; use appropriate guards
  and cleanup when plugin work can panic.
- Keep async/event-bus work separate from native thread requirements. Queue
  game-thread work where required; capture owned data rather than stale pointers.
- Use specific shared logging macros; do not log passwords or tokens.

## Validate

Test plugin behavior; verify native contracts against the target binary.

- Add behavioral tests for the new policy and relevant error/disabled paths.
- Do not add tests or snapshots asserting production native signatures, layouts,
  resolver keys, binding metadata, hook inventories, or declaration locations.
- Do not recreate native-binding boundary guards or baseline tables.
- Keep generic macro behavior tests separate from production native declarations.
- Build/test the Windows target with affected optional features on and off.
- Validate resolution and behavior in supported game builds; report limits.
- Report changed module paths, original-call behavior, activation/gates, and tests.
