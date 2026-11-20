---
name: create-detour
description: Create a plugin detour targeting an existing native runtime binding. Use when intercepting a Chivalry 2 function to add logging, publish events, or change behavior under src/unchained/patches.
---

# Creating a detour

Attach plugin behavior to a native binding without moving policy into the model.

- Follow [Creating game hooks](../../../CONTRIBUTING.md#creating-game-hooks),
  especially [Create the detour](../../../CONTRIBUTING.md#create-the-detour).
- Read [binding and trampoline contracts](../../../docs/native-bindings.md).

## Verify the binding

Load the `create-runtime-binding` skill before creating or changing the target.

- Use the skill tool when available; otherwise read the
  [runtime binding skill](../create-runtime-binding/SKILL.md).
- Reuse an existing verified method when possible.
- For a missing binding, follow that skill and its resolver prerequisite first.
- Confirm the method path, exact signature, resolver/registry key, feature gates,
  and supported builds.
- Do not target a resolver type, wrapper address, or generated vtable method.
  Virtual implementations need a separate resolver-backed binding.

## Choose the functional module

Place replacements under `src/unchained/patches/*` by plugin purpose.

- Reuse the relevant hosting, backend, networking, message, or other feature module.
- Put hooks that only log in `diagnostics.rs`.
- Do not mirror native classes or header directories in the patches tree.
- Register new modules in their parent `mod.rs`.
- Put imports first, then all detour entry points, then instruction patches,
  helpers, state, types, and tests.
- Gate a whole module when all its contents require the same feature.
- Keep native resolvers and ABI declarations under `src/chivalry2/*`.

## Implement the replacement

Use `bind_func_detour` on an unsafe free function.

- Target `NativeType::method`; names do not require a `_detour` suffix.
- Match every parameter and return type, including hidden storage and lifetimes.
- Declare an instance receiver explicitly as the first argument.
- Rust checks compatibility with invariant binding metadata; this does not
  establish the native ABI's correctness.
- Default activation is `ACTIVE`. Add a noncapturing condition only when needed.
- Conditions run at attachment and also require global auto-activation.
  Put per-call decisions in the body.
- Only one detour may own a native address; extend an existing replacement
  rather than installing a duplicate.
- Do not use legacy `CREATE_HOOK!` or `bind_resolved_func_detour`.

Alternative to the existing UDP replacement; do not install both:

```rust
use crate::chivalry2::engine::{FString, FUDPPing, TFunctionFIcmpEchoResult};
use crate::tools::hook_globals::cli_args;

#[unchained_macros::bind_func_detour(FUDPPing::udp_echo, || !cli_args().is_server())]
unsafe fn log_udp_echo(
    endpoint: *mut FString,
    timeout: f32,
    callback: *mut TFunctionFIcmpEchoResult,
) {
    sdebug!("UDP ping requested with timeout {}", timeout);
    unsafe { FUDPPing::udp_echo(endpoint, timeout, callback) };
}
```

## Preserve native behavior deliberately

Make original-call and lifetime decisions explicit.

- Call the native binding for the original implementation; attached bindings
  use the trampoline even when the detour is disabled.
- Do not call the detour recursively or dispatch through a vtable to reach
  the original; virtual dispatch may re-enter the replacement.
- Preserve return values unless changing them is the requested policy.
- Preserve pointer validity, thread affinity, ownership, and callback lifetimes.
- Do not free engine-owned borrowed buffers or retain pointers beyond validity.
- Do not let Rust panics unwind through native callers; use appropriate guards
  and cleanup when plugin work can panic.
- Keep async/event-bus work separate from native thread requirements. Queue
  game-thread work where required; capture owned data rather than stale pointers.
- Use specific shared logging macros; do not log passwords or tokens.

## Validate

Test the behavior and keep the production hook inventory reviewed.

- Add a reviewed row to `unchained-macros/impl/tests/native_detours.tsv` for each
  new production detour: target, resolver, native signature, condition, and gate.
- Add behavioral tests for the new policy and relevant error/disabled paths.
- Do not add redundant typed function-pointer assignment tests.
- Run the boundary guard:

  ```sh
  cargo test --locked -p unchained-macros-impl --test native_detour_boundary
  ```

- Build/test the Windows target with affected optional features on and off.
- Validate resolution and behavior in supported game builds; report limits.
- Report changed module paths, original-call behavior, activation/gates, and tests.
