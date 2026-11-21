---
name: create-runtime-binding
description: Declare a verified Chivalry 2 native runtime binding under src/Chivalry2, including instance/static ABIs, request-registry bindings, native globals, and vtable methods. Use when exposing a native function for direct calls or a plugin detour.
---

# Creating a runtime binding

Expose a verified native target without adding plugin policy to its model.

- Follow [native bindings](../../../CONTRIBUTING.md#declare-a-native-binding),
  [vtable methods](../../../CONTRIBUTING.md#declare-vtable-methods), and
  [source layout](../../../docs/engine-sources.md).
- Read [binding and trampoline contracts](../../../docs/native-bindings.md).

## Resolve the prerequisite

Load the `create-resolver` skill before creating or changing an address resolver.

- Use the skill tool when available; otherwise read the
  [resolver skill](../create-resolver/SKILL.md).
- Reuse a verified existing resolver rather than introducing duplicate scans.
- Require evidence from the latest target binary for new ABI assumptions.
- Confirm target identity, native owner, static/instance status, parameters,
  hidden return storage, and return convention.
- Stop and request evidence when the ABI is uncertain; a successful scan alone
  does not establish a callable signature.

## Place the declaration

Keep native contracts with their source owners.

- Use the appropriate `src/Chivalry2/{Engine,TBL,TBLLoadingScreen,Plugins}/*`
  module, not a plugin-policy directory.
- Preserve native module/header casing, including `Runtime`, `ThirdParty`, and
  `CoreUObject`; omit visibility directories. Add new modules to `mod.rs`.
- Put source evidence/imports first, then resolvers, then native types and impls.
- Reuse existing types; declare only layouts supported by evidence.
- Keep opaque types opaque. Use raw pointers where Rust borrow guarantees
  cannot be established.
- Preserve intentional integration aliases and feature restrictions.
- Update cache/registration consumers when explicitly renaming a resolver.
- Use C++ method spelling, including acronyms and `GetPost_*` names.
- Use `Destructor` for native C++ destructors; preserve deleting flags and returns.
- Keep `TBL` capitalized: `TBLFoo` and `GetTBLGameMode`, not `TblFoo`.
- Use one `Native source: <URL>` module comment for a verified SDK/UE header.
  Keep differing original paths in `Native owner:`; do not add an `SDK header:` line.
- Name project-owned method resolvers `Class__Method`, free functions `Function`, and value resolvers
  `Owner__Value` or `Value`; use `Class__Destructor` for destructors.
- Use descriptive overload suffixes only where Rust cannot share a name.
- Keep ABI/safety notes; omit comments that only repeat signatures.
- Keep detours, configuration policy, queues, and patch bytes out of native models.

## Declare the callable binding

Use an empty unsafe method under `runtime_bindings`.

- Mark header-backed native structs, enums, unions, and aliases with
  `#[unchained_macros::runtime_bindings]`, including generic/layout-only types.
  Type annotations add documentation only; they do not register callables.
  Put the marker before layout/code-generation attributes and retain their order.
  Do not mark Rust-only adapters, guards, views, errors, or callback aliases.
- Apply `#[unchained_macros::runtime_bindings]` to the native type's inherent impl.
- Apply `#[unchained_macros::bind_resolved_func(Resolver)]` to the method.
- Use the declared resolver type path, not a renamed import.
- `self::ResolverName` selects the current module's resolver; it is optional when
  unambiguous and is distinct from the type-relative `Self::`.
- Choose accessible visibility, usually `pub(crate)`.
- Preserve exact native argument order, widths, returns, and lifetimes.
- Keep static calls receiver-free. For instance calls, use a verified receiver
  such as `self: *mut Self`.
- Declare hidden C++ ABI arguments explicitly; macros do not infer them.
- Mark verified indirect-by-value object parameters `#[native_by_value]` on
  their `*mut T` ABI storage. This is source-selection metadata, not a copy or
  ownership operation; ordinary pointers must not carry it.
- Document pointer validity, ownership, threading, and callback-lifetime obligations.
- Match surrounding feature gates; do not widen optional native dependencies.
- Calls use the resolved entry before attachment, then the original trampoline.
  Calling a binding must not invoke its plugin replacement.
- Assume startup initialized required bindings.
- Use the shared preinit binding registry; do not add per-binding startup scans
  or a separate function-pointer collection in `Globals`.
- Keep plugin-owned mutable state behind ordinary methods, not native bindings.

Existing static-call pattern; reuse the existing declaration rather than
registering a second copy:

```rust
#[unchained_macros::runtime_bindings]
impl FUDPPing {
    #[unchained_macros::bind_resolved_func(FUDPPing__UDPEcho)]
    pub(crate) unsafe fn UDPEcho(
        endpoint: *const FString,
        timeout: f32,
        #[native_by_value] callback: *mut FIcmpEchoResultCallback,
    ) {}
}
```

## Handle other native targets

Choose the binding mechanism that matches the address source.

- **Patternsleuth:** pass the full resolver type path to `bind_resolved_func`.
- **Request registry:** use `bind_resolved_request_function(Key)` under
  `runtime_bindings`. Verify the producer supplies that exact key as an RVA in
  `OffsetRegisty`; do not put an absolute address into the registry.
- **Globals:** use `bind_resolved_value(Resolver, Address)` for an inline object's
  address or `Value` to read a `Copy` value from a slot. These are not detour targets.
  Declare native globals as module-level accessors under their declaring header;
  keep Rust convenience adapters separate from the native type's methods.
  Preserve the native name and pointer type; do not replace the global with an
  allocator handle, owning reference, or cached copy of the slot's contents.
- **Virtual calls:** use `rust_ctables::cpp_struct` and `#[vtable(...)]`.
  Load `create-vtable`, or read the [vtable skill](../create-vtable/SKILL.md),
  before creating or extending a table declaration.
  - Verify the vptr location and byte offsets; Windows x64 slots are eight bytes.
  - Use `#[inherit]` on the verified embedded base rather than adding another vptr.
  - Generated calls use the current vtable; they do not supply binding metadata
    for `bind_func_detour` or guarantee original-call bypass.
  - To detour a concrete virtual implementation, resolve its entry and declare
    a separate resolver-backed method.

## Validate and hand off

Check native correctness separately from Rust signature compatibility.

- Compile the binding on the supported Windows target.
- Test affected feature combinations, including disabled optional features.
- Test Rust helper behavior, such as conversion, traversal, collection operations,
  and ownership handling, where applicable.
- Do not add native signature, layout, slot, registration, resolver, or binding
  contract tests, including typed function-pointer assignments and snapshots.
- Verify native contracts with source/binary evidence, not synthetic fixtures
  that merely repeat the declaration.
- Do not recreate native-binding boundary guards or baseline tables.
- Report the module/method path, resolver or registry key, verified ABI,
  feature gates, evidence, and test results.
