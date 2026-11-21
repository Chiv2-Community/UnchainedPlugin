# unchained-macros

Runtime bindings, detours, patches, resolver declarations, and startup registration.
Procedural attributes live in `impl/`; runtime support lives in `src/`.
Native declarations and feature gates stay in the consuming plugin.

Start with [Creating game hooks](../CONTRIBUTING.md#creating-game-hooks).
See [native contracts](../docs/native-bindings.md), the
[resolver reference](resolvers.md), and [source documentation](../docs/engine-sources.md#source-links).

## Runtime support

Startup:

1. `startup_resolvers()` selects compiled binding/patch roots and explicit
   side-effect providers. Declarations alone do not cause scans.
2. Resolve them in one Patternsleuth context; transitive dependencies are automatic.
   Project declarations take precedence over upstream namesakes.
3. Initialize values and functions from cached/scanned RVAs using
   `initialize_value_bindings(base, &offsets)` and
   `initialize_function_bindings(base, &offsets)`.
4. Apply patches and attach detours before enabling replacements.

Use `require_resolver!(Name)` for a scan that populates an offset map or other
required side effects. Request-key bindings need their provider registered this way.
`supports_selection(name, key)` applies typed, consumer-defined restrictions.
The host owns selection state and decides which keys contribute patterns.

`ResolvedAddress` publishes addresses once; `FunctionBinding` refers to Retour's
detour rather than copying its trampoline address. Both binding inventories use
`BindingRegistration<T>`. Hook/patch callbacks receive the executable base.
Patch writes use the consumer's `crate::tools::memtools`; generated code also
requires the consumer's `retour` and `inventory` dependencies.

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
- `detour_is_initialized("Resolver")` and `detour_is_enabled("Resolver")` query
  Retour's state. Unknown names return `false`.
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

`define_custom_resolver!(Name, with(expression), |context| { ... })` evaluates
host-supplied context when resolving and permits `?`. This selects matching
behavior, not a restriction. Enum-keyed pattern dictionaries register their own
applicability; `Key: None` disables that key.
The plugin and BinaryBloodhound share one locked Patternsleuth dependency.

## Consumer-defined selections

```rust,ignore
#[unchained_macros::resolver_selection_key(require = [Read, Write])]
enum Mode { Read, Write, Shared }

define_pattern_resolver!(ReadEntry, select [current_mode(), Mode::Shared], {
    Mode::Read: ["AA BB"],
    Mode::Write: None,
    Mode::Shared: ["EE FF"],
});
```

`resolver_selection_key` supplies `SelectionKey`, `Clone`, `Copy`, `PartialEq`,
and `Eq` (without duplicating existing derives). It rejects non-enums, fields,
and generic parameters. Rust checks
variant names and enum types; duplicate dictionary keys are compile errors.
`SelectionKey::REQUIRED` exposes the required enum values. Omitting a
required key is a compile error; explicitly use patterns or `None`.

The first selection controls applicability. `None` disables the resolver before
pattern construction or inline execution, regardless of later shared keys.
`[]` stays enabled and contributes no local patterns. Omitted optional keys stay
enabled and contribute nothing. Plain pattern lists apply to every selection.

The ordered selection list is explicit: no key is a built-in fallback or shared
case. Repeated selections append their patterns again. Restrictions use enum
type identity and equality, not discriminants. Unknown names remain unrestricted;
conflicting restrictions or a different key type fail closed.

The host filters scan roots, hooks, and patches with `supports_selection`.
Use `initialize_function_bindings_where` and `initialize_value_bindings_where`
with the same predicate so disabled bindings stay uninitialized, even with cached
offsets. Set host state before scanning; the toolkit owns no global selection.
Use `CREATE_PATCH!(..., IF { ... })` for other host-defined patch conditions.

## Validation

```sh
cargo test --locked -p unchained-macros -p unchained-macros-impl
```

Tests cover generic declaration rejection, local native-call fixtures, resolver
selection, and lifetime/mutability constraints. Windows fixtures also attach real
detours and check interception and original-call bypass.
They do not establish game signatures, layouts, addresses, or resolver contracts.
