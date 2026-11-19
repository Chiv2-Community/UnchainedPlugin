# Native bindings and plugin detours

Native models declare callable functions. Plugin modules declare replacements.
All 45 production detours use this split, including optional feature-gated hooks.

## Native model

Keep the resolver, ABI, and binding with the native owner:

```rust
#[unchained_macros::runtime_bindings]
impl FUDPPing {
    #[unchained_macros::bind_resolved_func(UDPEcho)]
    pub unsafe fn udp_echo(
        endpoint: *mut FString,
        timeout: f32,
        callback: *mut TFunctionFIcmpEchoResult,
    ) {}
}
```

`runtime_bindings` emits typed metadata alongside direct bindings. This metadata
is internal macro plumbing, not an application API. Method visibility controls
whether another module can target its binding.

## Plugin replacement

Target the method, not its resolver or Rust wrapper address:

```rust
#[unchained_macros::bind_func_detour(FUDPPing::udp_echo, || !cli_args().is_server())]
unsafe fn ipv6_udp_echo(
    endpoint: *mut FString,
    timeout: f32,
    callback: *mut TFunctionFIcmpEchoResult,
) {
    // Apply plugin policy, then invoke the original.
    unsafe { FUDPPing::udp_echo(endpoint, timeout, callback) };
}
```

- Replacements are unsafe free functions; names need no `_detour` suffix.
- Instance bindings use an explicit first receiver argument in the replacement.
- Rust checks the native signature, including receiver and return lifetimes,
  against invariant typed binding metadata.
- Activation conditions and global auto-activation retain their existing behavior.
- Diagnostics and state queries retain the binding's resolver/request-key name.
- Request bindings under `runtime_bindings` expose the same metadata; replacements
  need not know whether the target came from a resolver or request registry.

## Original-call contract

| Binding state | Model method calls |
| --- | --- |
| Uninitialized | Panic before a native call |
| Bound, no attached detour | Resolved native entry |
| Attached, enabled or disabled | Original-function trampoline |

Calling the model method from its replacement does not recurse into that
replacement. It bypasses our hook for this target, not hooks on nested callees
or arbitrary third-party patches.

Startup must:

1. Resolve and initialize **all** function bindings, including aliases.
2. Attach detours while plugin binding callers are quiescent.
3. Publish each trampoline to every binding sharing the native entry.
4. Enable the patch only after publication.

Atomic publication does not make concurrent hot attachment safe: a caller may
already have loaded the old entry address. The API does not support dropping or
unloading detours/trampolines; their storage lives for the process lifetime.

Only one detour may own a native address. Aliases and legacy registrations use
the same address reservation, so a second installation fails rather than
silently chaining patches. Failed construction releases its reservation; once
the detour is stored, its address remains reserved even if activation fails.

## Current files

- Model: `src/chivalry2/engine/runtime/online/icmp/icmp.rs`
- Plugin policy: `src/unchained/detours/networking/ipv6_support.rs`
- Other native models: `src/chivalry2/{engine,tbl,tbl_loading_screen}/`
- Native game-plugin ABIs: `src/chivalry2/plugins/`
- Custom blueprint layouts/adapters: `src/unchained/blueprints/`
- Replacements: `src/unchained/detours/{engine,tbl,tbl_loading_screen,plugins}/`
- Plugin-only chat helpers, game-thread jobs, mounted-pak tracking, and override
  state live with the replacements, not on native types.
- `TBLUtility` and `UGameplay` adapt native namespaces for typed bindings, not
  UObject instances. Their native free functions have no receiver.
- Typed binding state: `unchained-macros/src/binding.rs`
- Declaration expansion: `unchained-macros/impl/src/function_detour.rs`
- Real native-patching tests: `unchained-macros/tests/detached_hooks.rs`
- Migration baseline and boundary guard:
  `unchained-macros/impl/tests/native_detour_boundary.rs` and `native_detours.tsv`

The boundary guard checks all native signatures, resolver keys, activation
conditions, and feature gates against the migration baseline. Production code
cannot reintroduce legacy detour declarations or colocate replacements with
native models. Legacy macro support remains for compatibility tests.

Native bindings always bypass replacements. Code that deliberately needs plugin
policy, such as capturing a synthetic console message, calls the plugin helper
explicitly rather than a detour method on the native model.
