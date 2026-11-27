# Native bindings and plugin detours

## Contents

- [Native model](#native-model)
- [Native statics](#native-statics)
- [Plugin replacement](#plugin-replacement)
- [Original-call contract](#original-call-contract)
- [Attachment lifecycle](#attachment-lifecycle)

## Native model

Declare callable native functions separately from plugin behavior.

Import bindings, detours, patches, and selectors from `unchained_macros_core`.
Import scan and offset-provider declarations from `unchained_macros_patternsleuth`,
or reflection declarations from `unchained_macros_kismet`. Binding, hook, and patch
types require an explicit backend; discovery uses `*_inventory_for::<Backend>()`.

- Keep resolvers, native ABI declarations, and bindings under `unchained-plugin/src/Chivalry2/*`.
- Follow the [native source layout](engine-sources.md).
- Use `#[runtime_bindings]` on the native type's inherent impl.
- Declare an empty unsafe method with `#[bind_resolved_func(Resolver)]`.
- Keep static methods receiver-free; preserve instance receivers and hidden ABI arguments.
- Match C++ method spelling, including acronyms and names such as `GetPost_GetMotd`.
- Use `Destructor` for native destructors without changing deleting flags or returns.
- Preserve `TBL` capitalization; name project-owned method resolvers `Class__Method`
  and free-function resolvers `Function`.
- `self::ResolverName` qualifies a module-local resolver; use it when needed to
  disambiguate imports. It is not a macro requirement.
- Keep comments about ABI or safety, not duplicate function headers.
- Use visibility accessible to the replacement.
- `runtime_bindings` emits invariant signature metadata; it is internal plumbing,
  not an application API.
- Provider offsets use `define_offset_resolver!(Key)` and ordinary
  `bind_resolved_func(Key)` declarations with the same metadata.
- `TBLUtility` and `UGameplay` adapt native namespaces, not UObject instances.
- See [Creating game hooks](../CONTRIBUTING.md#creating-game-hooks) for the full workflow.

Existing static UDP-ping binding:

```rust
#[unchained_macros_core::runtime_bindings]
impl FUDPPing {
    #[unchained_macros_core::bind_resolved_func(FUDPPing__UDPEcho)]
    pub(crate) unsafe fn UDPEcho(
        endpoint: *const FString,
        timeout: f32,
        #[native_by_value] callback: *mut FIcmpEchoResultCallback,
    ) {}
}
```

## Native statics

Mirror C++ static/global access for every native global used by this project.

- Preserve native spelling and the declaring header, e.g. `GMalloc` in
  `HAL/MemoryBase.rs`, `GUObjectArray` in `UObject/UObjectArray.rs`.
- Declare `#[bind_resolved_value(Resolver, Value)] pub static GMalloc: *mut FMalloc;`.
  Use `Address` instead for inline native storage such as `GUObjectArray`.
- Read the current pointer explicitly: `unsafe { GMalloc.read().Malloc(size, alignment) }`.
  rust-ctables supplies vtable dispatch; the binding generates no methods.
  `Value` reads the current slot contents, including after UE replaces the pointer.
  `Address` returns the inline object's pointer without copying its storage.
- For ordinary reference-receiver methods, borrow only after reading the pointer:
  `unsafe { (&*GUObjectArray.read()).objects() }`.
- Binding stores only the resolved address. It adds no locks, cached allocator,
  ownership, `Deref`, or `DerefMut`. Raw native receivers retain pointer provenance.
- Native storage/lifetime/method contracts and synchronization remain required.
  One-time address publication does not synchronize ordinary C++ pointer writes.
  Inline registry guards still acquire the engine's critical section.
- Keep plugin-owned state separate; see [macro usage](../unchained-macros-core/README.md#native-globals).

## Plugin replacement

Reflection-backed native C++ methods can use
`unchained_macros_kismet::kismet_runtime_bindings("/Script/Package.Class")`.
They retain the same unsafe declarations and detour API, but initialize on the
game thread once reflection is available. See
[runtime reflection bindings](kismet-runtime-bindings.md) for lifecycle and limits.

Target the native binding from a functional module under `unchained-plugin/src/unchained/patches/`.

- Use `#[bind_func_detour(Type::Method)]` on an unsafe free function.
- Do not target the resolver or Rust wrapper address.
- Declare instance receivers as the first argument.
- Names need no `_detour` suffix.
- The compiler checks argument types, returns, and lifetime relationships against
  the binding. It does not verify the ABI against the binary.
- `#[mutability_override]` on a pointer/reference replacement parameter allows
  either outer mutability, including an exact match, checked against the binding.
  Pointees, nested mutability, other parameters, returns, and lifetimes stay exact.
  Raw pointers and references cannot be interchanged. Every intercepted argument
  must satisfy the detour's validity, aliasing, and write-access contract:
  `&mut T` requires exclusive access for its entire borrow, even for native `&T`.
  The annotation does not make shared or read-only memory writable.
- Hooks default to enabled; custom conditions run at attachment.
- Activation also requires global auto-activation.
- State queries and hook diagnostics use the Rust detour function name.
  Each hook references its native binding; that binding keeps the native target
  name separate from its resolver descriptor or provider offset key.
- Put logging-only replacements in `diagnostics.rs`.
- Put all detours after imports, before instruction patches, helpers, state, and tests.
- Keep plugin queues and override state with plugin behavior, not native types.

Alternative to the existing UDP replacement:

```rust
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

## Original-call contract

Call the native binding to invoke the original implementation.

- **Uninitialized:** panic before making a native call.
- **Bound, no attached detour:** call the resolved entry.
- **Attached, enabled or disabled:** call the original-function trampoline.
- Calling the binding bypasses that method's detour and entry/exit listeners.
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
2. Attach the shared dispatcher while plugin binding callers are quiescent.
3. Publish a reference to the Retour detour to every binding sharing its native
   entry. Original calls read its trampoline; no second trampoline address is stored.
4. Activate individual callbacks only after publication.

- One physical dispatcher owns a native address. A native binding supports one
  behavior-changing detour, one entry listener, and one exit listener.
  Dummy exec markers support one entry listener only.
- Conditions are attachment-time activation gates. Each callback has independent
  logical activation; disabling a detour does not disable its listeners.
- Use registration `set_enabled` controls for logical activation. Disabling a raw
  physical Retour hook disables every callback sharing it.
- Aliases share address reservations; duplicate attachment fails rather than
  silently chaining hooks.
- Failed construction releases its reservation.
- A stored detour retains its reservation even if activation fails.
- One-time publication does not make concurrent attachment safe; a caller may
  already have loaded the old entry.
- Detours and trampolines live for the process lifetime; unloading is unsupported.

Resolved addresses use `OnceLock`, not manually managed atomic address slots.
Startup writes once; cross-thread readers still require safe publication.
Code and data share `ResolvedAddress`; data bindings carry no detour state.

## Observation listeners

For an illustrative native binding `unsafe fn Method(value: u32) -> bool`:

```rust,ignore
#[unchained_macros_core::bind_func_entry_listener(NativeType::Method)]
unsafe fn entered(value: &u32) {
    // Read the input; do not change native execution.
}

#[unchained_macros_core::bind_func_exit_listener(NativeType::Method)]
unsafe fn returned(value: &u32, result: &bool) {
    // Entry-time input snapshot and actual caller-visible result.
}
```

- Callbacks return `()` and take immutable, call-scoped borrows. Exit appends a
  borrowed result, including `&()` for void. Native pointer inputs are observed
  as pointer values, not implicitly dereferenced objects.
- Exit input types require `ObserverSnapshot`. Copy values have a default
  implementation; native FString copies its array, and FText retains a shared
  native reference. Raw pointer snapshots do not extend pointee lifetimes.
  Retained FText history is not a frozen display string.
- Native reference inputs cannot currently be snapshotted by the generic exit
  attribute. Unsupported ownership/lifetime contracts fail compilation.
- Entry precedes detour/original execution. Exit observes the final result even
  when a detour replaces it or skips the original. Neither observer can replace
  arguments, results, or dispatch through its API.
- Callback-induced reentry suppresses observers, not execution. Ordinary recursive
  calls remain observable. Rust observer/condition panics are isolated; native
  exceptions, aborts, and non-returning calls do not promise exit events.
- Unsafe callbacks must preserve native ownership, aliasing, thread affinity, and
  pointee lifetimes. Immutable pointer observations do not authorize pointee writes.
- Native coverage follows an entry address, including its aliases. Exec-only
  Blueprint observations require a verified unique wrapper and preserve real VM
  stepping and cleanup. Dummy markers reject exit listeners and detours; see
  [Kismet bindings](kismet-runtime-bindings.md).
