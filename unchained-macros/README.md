# unchained-macros

Runtime bindings, detours, patches, resolver declarations, and startup registration.
Procedural attributes live in `impl/`; runtime support lives in `src/`.
Native declarations and feature gates stay in the consuming plugin.

Start with [Creating game hooks](../CONTRIBUTING.md#creating-game-hooks).
See [native contracts](../docs/native-bindings.md), the
[resolver reference](resolvers.md), and [source documentation](../docs/engine-sources.md#source-links).

## Runtime support

Startup:

1. Initialize project selector state, then call `SelectorSnapshot::capture_registered()`.
   Each provider is read once; values and fallback chains are frozen.
2. `startup_resolvers(&snapshot)` selects compiled binding/patch roots and explicit
   side-effect providers. It validates coverage and selector availability without
   evaluating patch factories or runtime conditions.
3. Resolve roots with `snapshot.resolve_many(&image, &factories)`. Transitive
   dependencies share the context snapshot; project declarations override upstream namesakes.
4. Include the complete `snapshot.cache_key()` in cache identity and validate it
   when loading. A file-name hash alone is not sufficient.
5. Use `binding.supports(&snapshot)` with `initialize_function_bindings_where`;
   value initialization still uses `snapshot.supports_resolver(name)`.
   Pass the same snapshot to `apply_registered_patches` and use it for hook filtering.

Use `require_resolver!(Name)` for a scan that populates an offset map or other
required side effects. Request-key bindings need their provider registered this way.
The host owns selector state. Different resolvers may use different providers;
there is no project-global enum in the toolkit.

`ResolvedAddress` publishes addresses once; `FunctionBinding` refers to Retour's
detour rather than copying its trampoline address. Both binding inventories use
`BindingRegistration<T>`. Hooks receive the executable base; patch factories
receive the resolved native address. Hosts implement `PatchWriter`; generated code also
requires the consumer's `retour` and `inventory` dependencies.

Generated resolvers expose `ResolverDeclaration::DESCRIPTOR`: a stable
`ResolverDescriptor` containing Patternsleuth's `NamedResolver` and an optional
selection filter. Scans and consumers use the same frozen policy.
`FunctionBinding` records the native target name separately from its resolver
cache key. `FunctionSource` distinguishes a resolver descriptor from a provider
offset key. `HookRegistration` references that binding and names the Rust detour.
`PatchRegistration` names the patch item and references its resolver descriptor.

Adapt a dependency's single-address tuple resolver without changing its scan:

```rust,ignore
define_custom_resolver!(ExternalName, delegate dependency::ExternalName);
```

## Direct method bindings

```rust,ignore
#[unchained_macros::runtime_bindings]
impl NativeObject {
    #[unchained_macros::bind_resolved_func(ReadValue)]
    pub unsafe fn Read(&self) -> &i32 {}
}
```

- Declare an empty unsafe body and a verified Windows x64 `extern "C"` ABI.
  Supports borrowed/raw receivers, static/free functions, and lifetime parameters.
- Type/const generics, async/const functions, explicit ABIs, and nonempty bodies
  are rejected. Hidden native parameters must be declared explicitly.
- Resolver arguments are type paths; the final identifier is the cache key.
  Do not use an import alias with a different name.
- `bind_resolved_request_function(GetPost_Name)` uses an offset-provider key,
  not a Rust resolver type.
- Calls use the resolved entry before attachment, then the original trampoline,
  even when the detour is disabled. Missing bindings fail startup; early calls panic.

## Native bindings and detached detours

Put native declarations under `src/Chivalry2/*` and replacements under
`src/unchained/patches/*`. Replacements target the binding, not its resolver:

```rust,ignore
#[unchained_macros::bind_func_detour(NativeObject::Read)]
unsafe fn read<'a>(object: &'a NativeObject) -> &'a i32 {
    unsafe { object.Read() }
}
```

Calling the binding from its replacement bypasses that replacement.
Invariant metadata checks argument types, returns, and lifetime relationships.
It does not verify native ABI/layout, extend lifetimes, or synchronize patching.
Keep callers quiescent during attachment; detours and trampolines live for the process.
See the [attachment lifecycle](../docs/native-bindings.md#attachment-lifecycle).

## Detour conditions and state

- Defaults to `ACTIVE`. `INACTIVE`, `CALLED`, and `DISABLED` mean passive attachment.
  A noncapturing closure/function may supply a runtime condition.
- Activation requires both global auto-activation and the condition at attachment.
- `detour_is_initialized("read")` and `detour_is_enabled("read")` query
  Retour's state by Rust detour function name. Unknown names return `false`.
- Native calls must not unwind through the detour.

## Lifetime-aware hooks

`bind_func_detour` preserves higher-ranked signatures before erasing pointers
for Retour. Keep native borrows and their relationships explicit where needed.

`#[mutability_override]` adapts only a parameter's outer pointer/reference
mutability, in either direction or as an exact match. Pointees, nested mutability,
unmarked parameters, returns, and lifetimes stay exact. Raw pointers and references
cannot be interchanged; spell the outer type directly rather than using an alias.

The caller must still prove validity, aliasing, and write access. In particular,
`&mut T` requires exclusive access for the entire borrow, even for native `&T`.
The annotation does not make shared or read-only memory writable.
See [replacement contracts](../docs/native-bindings.md#plugin-replacement).

## Required native values

```rust,ignore
#[unchained_macros::bind_resolved_value(patternsleuth::resolvers::unreal::gmalloc::GMalloc, Value)]
pub static GMalloc: *mut FMalloc;
```

- `Value` reads the current pointer from its slot on every call.
  `Address` uses an inline object's address without copying it, e.g. `GUObjectArray`.
- Mark the target struct `runtime_bindings(global)` before `cpp_struct`.
  Forwarders come from its vtable declarations or similarly marked public inherent impl.
  Other native type markers add documentation only.
- All forwards are unsafe and preserve receiver/return lifetime relationships.
  Use `unsafe { GMalloc.Malloc(size, alignment) }`; `as_ptr()` is for APIs requiring
  a pointer, not for caching a replaceable native pointee.
- The proxy owns no native object and exposes no `Deref`/`DerefMut`.
  Binding caches only the address, adds no per-call lock, and does not synchronize
  C++ pointer writes. Native guards retain their engine locks.
- Invalid/missing bindings and null pointers panic. Binding before the engine
  populates a slot is allowed; zero scalar values are valid.
- Zero-argument unsafe accessors support generic `Copy` values and raw inline
  addresses without implicit arithmetic or method forwarding.

Keep native spelling and declaring-header ownership; plugin-maintained state
is separate. See [native statics](../docs/native-bindings.md#native-statics).

## Custom resolvers

```rust,ignore
use binary_bloodhound::BinaryMatcher;
use unchained_macros::define_custom_resolver;

define_custom_resolver!(FunctionEntry, {
    BinaryMatcher::patterns(["AA BB CC DD"])?
        .root_functions()
        .addresses()
        .ensure_one()
});
```

Patterns here are illustrative, not verified game signatures.
The matcher must take no input and produce exactly one `u64` address.
Construction/evaluation errors retain resolver context; only the final address
is cached, not matcher provenance.

`define_custom_resolver!(Name, using Selector, |value| { ... })` reads its value
from the context snapshot and permits `?`. The explicit `with(expression)` form
still accepts arbitrary host context, but does not automatically track it for caches.
Any address-affecting selector should use a registered provider.
Custom context chooses matching behavior, not an installation restriction;
enum-keyed pattern dictionaries register applicability through `Key: None`.
The plugin and BinaryBloodhound share one locked Patternsleuth dependency.

## Consumer-defined selections

```rust,ignore
#[unchained_macros::resolver_selection_key(require = [Read, Write], fallback = Other)]
enum Mode { Read, Write, Other }

impl ResolverSelector for Mode {
    fn current() -> Self { current_mode() }
}

define_pattern_resolver!(ReadEntry, {
    Mode::Read: ["AA BB"],
    Mode::Write: None,
    Mode::Other: ["EE FF"],
});
```

`resolver_selection_key` supplies `SelectionKey`, `Clone`, `Copy`, `PartialEq`,
and `Eq` (without duplicating existing derives). It rejects non-enums, fields,
and generic parameters. Rust checks
variant names and enum types; duplicate dictionary keys are compile errors.
`SelectionKey::REQUIRED` exposes the required enum values. Omitting a
required key is a compile error; explicitly use a nonempty pattern list or `None`.
The `fallback = Other` option supplies a default `ResolverSelector::fallback(self)`:
other variants return `Some(Other)` and `Other` returns `None`. Override the method
for longer chains. Cycles are rejected when capturing a snapshot.

The first declared entry on the frozen chain wins. Only missing keys advance to
fallback; failed scans do not. `None` disables resolution before constructing
patterns or running inline bodies. Empty lists are compile errors.
Plain pattern lists apply to every selection.

Qualified keys identify the provider, so neither `select` arguments nor host
wrapper macros are needed. Restrictions use enum type identity, not discriminants.
Unknown resolver names remain unrestricted; missing providers fail before startup.

The host filters resolver applicability with `snapshot.supports_resolver`.
Explicit patch selections narrow only that patch, not its resolver or other consumers.
Use `initialize_function_bindings_where(..., |binding| binding.supports(&snapshot))`
and `initialize_value_bindings_where(..., |name| snapshot.supports_resolver(name))`
so disabled bindings stay uninitialized, even with cached
offsets. Context clones retain their cached snapshot even when polled on another
thread. New values require a new session and matching cache identity; native
bindings still publish addresses once.
Patch factories can return `Patch::bytes(...).when(condition)` for runtime gates.

## Instruction patches

Import the target resolver type. `register_patch` accepts a factory or an
immutable `Patch` static. An explicit key cannot be disabled by the resolver;
an unqualified patch requires unrestricted coverage. These checks are compile-time.

```rust,ignore
#[register_patch(FunctionEntry, Mode::Read)]
static READ_PATCH: Patch = Patch::bytes(0x4D, &[0x90, 0xE9]);
```

Inventory IDs are patch item names. Each resolver's patches jointly cover its
enabled required keys; resolver `None` entries remove those requirements.
Aggregate coverage and duplicate IDs are validated before scanning or writing.
See the [patch reference](patches.md) for factories, statics, conditions, and examples.

## Validation

```sh
cargo test --locked -p unchained-macros -p unchained-macros-impl
```

Tests cover generic declaration rejection, local native-call fixtures, resolver
selection, and lifetime/mutability constraints. Windows fixtures also attach real
detours and check interception and original-call bypass.
They do not establish game signatures, layouts, addresses, or resolver contracts.
