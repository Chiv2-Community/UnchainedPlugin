# Native bindings and plugin detours

## Contents

- [Native model](#native-model)
- [Plugin replacement](#plugin-replacement)
- [Original-call contract](#original-call-contract)
- [Attachment lifecycle](#attachment-lifecycle)

## Native model

Declare callable native functions separately from plugin behavior.

- Keep resolvers, native ABI declarations, and bindings under `src/Chivalry2/*`.
- Follow the [native source layout](engine-sources.md).
- Use `#[runtime_bindings]` on the native type's inherent impl.
- Declare an empty unsafe method with `#[bind_resolved_func(Resolver)]`.
- Keep static methods receiver-free; preserve instance receivers and hidden ABI arguments.
- Match C++ method spelling, including acronyms and names such as `GetPost_GetMotd`.
- Use `Destructor` for native destructors without changing deleting flags or returns.
- Preserve `TBL` capitalization; name project-owned resolvers `Class__Name` or `Name`.
- `self::ResolverName` qualifies a module-local resolver; use it when needed to
  disambiguate imports. It is not a macro requirement.
- Keep comments about ABI or safety, not duplicate function headers.
- Use visibility accessible to the replacement.
- `runtime_bindings` emits invariant signature metadata; it is internal plumbing,
  not an application API.
- Request bindings use `bind_resolved_request_function(Key)` and expose the same
  metadata.
- `TBLUtility` and `UGameplay` adapt native namespaces, not UObject instances.
- See [Creating game hooks](../CONTRIBUTING.md#creating-game-hooks) for the full workflow.

Existing static UDP-ping binding:

```rust
#[unchained_macros::runtime_bindings]
impl FUDPPing {
    #[unchained_macros::bind_resolved_func(UDPEcho)]
    pub(crate) unsafe fn UDPEcho(
        endpoint: *mut FString,
        timeout: f32,
        callback: *mut TFunctionFIcmpEchoResult,
    ) {}
}
```

## Plugin replacement

Target the native binding from a functional module under `src/unchained/patches/`.

- Use `#[bind_func_detour(Type::Method)]` on an unsafe free function.
- Do not target the resolver or Rust wrapper address.
- Declare instance receivers as the first argument.
- Names need no `_detour` suffix.
- The compiler checks argument types, returns, and lifetime relationships against
  the binding. It does not verify the ABI against the binary.
- Activation defaults to `ACTIVE`; custom conditions run at attachment.
- Activation also requires global auto-activation.
- State queries and diagnostics use the binding's resolver/request-key name.
- Put logging-only replacements in `diagnostics.rs`.
- Put all detours after imports, before instruction patches, helpers, state, and tests.
- Keep plugin queues and override state with plugin behavior, not native types.

Alternative to the existing UDP replacement:

```rust
#[unchained_macros::bind_func_detour(FUDPPing::UDPEcho, || !cli_args().is_server())]
unsafe fn log_udp_echo(
    endpoint: *mut FString,
    timeout: f32,
    callback: *mut TFunctionFIcmpEchoResult,
) {
    sdebug!("UDP ping requested with timeout {}", timeout);
    unsafe { FUDPPing::UDPEcho(endpoint, timeout, callback) };
}
```

## Original-call contract

Call the native binding to invoke the original implementation.

- **Uninitialized:** panic before making a native call.
- **Bound, no attached detour:** call the resolved entry.
- **Attached, enabled or disabled:** call the original-function trampoline.
- Calling the binding from its replacement does not recurse into that replacement.
- Bypass applies to our hook on that target, not nested callees or third-party patches.
- To invoke plugin policy deliberately, call the plugin helper explicitly.
- Assume startup initialized required bindings.
- Generated vtable calls dispatch through the current object table; they do not
  guarantee original-call bypass.
- Vtable methods are not `bind_func_detour` targets. Resolve a concrete
  implementation and declare a separate runtime binding to intercept it.
- See [vtable examples](../CONTRIBUTING.md#declare-vtable-methods).

## Attachment lifecycle

Initialize bindings and publish trampolines before enabling replacements.

1. Resolve and initialize **all** function bindings, including aliases.
2. Attach detours while plugin binding callers are quiescent.
3. Publish each trampoline to every binding sharing the native entry.
4. Enable replacements only after publication.

- Only one detour may own a native address.
- Aliases and legacy registrations share address reservations; duplicate attachment
  fails rather than silently chaining hooks.
- Failed construction releases its reservation.
- A stored detour retains its reservation even if activation fails.
- Atomic publication does not make concurrent attachment safe; a caller may
  already have loaded the old entry.
- Detours and trampolines live for the process lifetime; unloading is unsupported.
