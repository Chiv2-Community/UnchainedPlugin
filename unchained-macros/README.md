# unchained-macros

Owns generic detour generation, native binding
initialization, runtime registries, storefront filtering, and resolver macros.

Native declarations and macro invocations stay in the plugin. Generated
registrations reference this crate directly, not a consumer `resolvers` module.
Feature checks remain at the call site.

## Runtime support

- `HookRegistration`, `PatchRegistration`, and `OffsetRegisty` own the inventories.
- `initialize_function_bindings(base, &offsets)` and
  `initialize_value_bindings(base, &offsets)` initialize native bindings.
- Hook and patch callbacks take the executable base as their first argument.
  The host discovers that base; this crate does not query Windows.
- Hook callbacks use initialized bindings, not offset maps. Hook registration
  also provides initialized/enabled state queries.
- `PLATFORM` stores the host-selected storefront once. `current_platform()`
  and `supports_platform(name, platform)` serve resolver and startup filtering.
- Patch operations still use the consumer's `crate::tools::memtools` backend.
- See the [resolver reference](resolvers.md).

Logging, rich text, and gameplay-specific macros stay in the plugin; other
procedural attributes stay in `sleuth_macros`. Tests exercise generated hook
signatures and lifetime-aware attachment against local fixture functions,
not game functions.

## Custom resolvers

Place the resolver beside its native binding:

```rust,ignore
use binary_bloodhound::BinaryMatcher;
use unchained_macros::define_custom_resolver;

define_custom_resolver!(FText__AsCultureInvariant, {
    BinaryMatcher::patterns([
        "41 83 4E 10 02 4C 8D 5C 24 60 49 8B 5B 20 49 8B C6",
    ])?
    .root_functions()
    .addresses()
    .ensure_one()
});
```

- Accepts any no-input `Matcher` resolving to `ExactlyOne` `u64` address.
- Use `.addresses()` for typed address values and `.ensure_one()` to require
  one unique result. Missing or conflicting addresses are errors.
- Matcher construction may use `?`; construction and evaluation errors retain
  the resolver name through patternsleuth's normal error context.
- Uses the existing resolver inventory, per-context cache and native binding
  initialization. Only the final address is stored, not the matcher's provenance.
- The plugin and BinaryBloodhound share one patternsleuth Git source.
  `Cargo.lock` pins it to `3de6cfae`; use locked builds.

Use the platform-aware form to select a matcher at resolution time:

```rust,ignore
define_custom_resolver!(PlatformFunction, |platform| {
    let pattern = match platform {
        unchained_macros::PlatformType::STEAM => "AA BB",
        unchained_macros::PlatformType::EGS => "CC DD",
        _ => return Err(patternsleuth::resolvers::ResolveError::new_msg("Unsupported platform")),
    };
    BinaryMatcher::patterns([pattern])?.ensure_one()
});
```

The body receives `unchained_macros::current_platform()` and may use `?`.
It still must produce one `u64` address. This selects matcher behavior; it
does not register a platform restriction for hooks or patches.

## Native bindings and detached detours

New production hooks declare a native binding under `#[runtime_bindings]` in
`src/Chivalry2/*`, then target it with `#[bind_func_detour(Type::Method)]` from
`src/unchained/patches/*`. See
[Creating game hooks](../CONTRIBUTING.md#creating-game-hooks) for resolver choices,
binding declarations, replacement examples, and validation.

Native binding docs identify the SDK/UE source, resolver, its declaration
(80 characters maximum), and linked detours with their complete declarations.
Detour function docs show their linked binding and full native signature.
Request bindings identify their offset-registry key, not a nonexistent resolver.
Detours apply only when installed and enabled; direct binding calls bypass them.
Source links come from a single `Native source:` comment. Configure GitHub or
local checkout roots with `ARGON_SDK_SOURCE_LOCATION` and `UE_SOURCE_LOCATION`
in Cargo configuration; see [source links](../docs/engine-sources.md#source-links).
The Rust build script generates these documents and the macros include them as
standard Rustdoc. LSP clients and `cargo doc` receive the same content; no editor
configuration performs source lookup or substitution. Hover method, type, and
detour symbols; macro names themselves retain generic macro documentation.
Build-only generation, parsing, and developer examples live in
`build/unchained-native-docs`; root `build.rs` delegates to `build/mod.rs`.
Each generation shares a module/file index and caches extracted C++ declarations,
discarding syntax trees after indexing. The cache is build-local, so native source
edits are read again on the next generation.

Ambiguous C++ names are filtered using the binding's parameter types and receiver
constness. Native alias names, pointer/reference constness, primitive widths,
and generic type arguments participate. Pointer/reference and indirect-by-value
forms can remain indistinguishable; the matcher never selects the first match.
For a verified by-value native object lowered to a `*mut T` ABI parameter, mark
the binding parameter `#[native_by_value]`. This records existing indirection;
it does not copy the object, transfer ownership, or change generated ABI types.
Unmarked raw pointers are not guessed to be by-value objects.

For lowering that cannot be inferred, such as explicit hidden return storage,
provide an exact source-level selector:

```rust
/// Native overload: `(const FString&)`
```

Explicit selectors compare C++ parameter types and trailing cv/ref qualifiers,
ignoring parameter names and default values. Ambiguous matches remain unavailable.
The `native-implementation` example also
accepts selectors in `--target 'FText::AsCultureInvariant(const FString&)'`.
Use the source's type spelling; typedef equivalence and complex callable
parameter declarators are not resolved.

For a verified generated RPC wrapper, a binding may name its related body:

```rust
/// Native RPC implementation: `APlayerController::ClientMessage_Implementation`
```

When the wrapper body is unavailable, docs show this as
**Related RPC implementation**. This does not rename,
retarget, or bypass the bound wrapper. An available wrapper body takes precedence;
the generator never infers `_Implementation` automatically.
Local roots enable inline C++ implementations and complete native type
definitions. Unconfigured roots show the configuration note; unavailable
definitions show `No cpp sources available`.
Header-backed native structs, enums, unions, and aliases also carry
`#[unchained_macros::runtime_bindings]` directly. On a type, the macro only adds
source documentation; generic parameters, layouts, derives, and aliases are
unchanged. Callable registration still belongs to concrete inherent impls.
Rust-only views, guards, errors, and callback aliases are not native type markers.

Generated metadata and attachment helpers stay out of those hover spans; inspect
the expanded macro to view them. Link-opening behavior belongs to the editor.

An installed, enabled detour intercepts native game-code calls. Calling the Rust
native binding invokes the original implementation, not the replacement.

## Detour conditions and state

- `bind_func_detour` defaults to `ACTIVE`. Accepts `ACTIVE`, `INACTIVE`,
  `CALLED`, `DISABLED`, or a noncapturing closure/function returning `bool`.
- Activation requires both the global activation flag and the condition at attachment.
- The consuming crate supplies `retour` and `inventory`.
- Native calls must not unwind through the detour.

Direct empty unsafe stubs use `#[bind_resolved_func(Resolver)]`.

`detour_is_initialized("Resolver")` and `detour_is_enabled("Resolver")` expose
generated detour state without exposing storage. Unknown names return `false`.
Disabled detours still provide their original trampoline.
Assume startup initialized required bindings.

## Required native values

```rust,ignore
#[unchained_macros::bind_resolved_value(patternsleuth::resolvers::unreal::gmalloc::GMalloc, Value)]
pub static GMalloc: *mut FMalloc;

// GUObjectArray is an inline object, not a pointer slot.
#[unchained_macros::bind_resolved_value(patternsleuth::resolvers::unreal::guobject_array::GUObjectArray, Address)]
pub static GUObjectArray: *mut FUObjectArray;
```

- Mirror every C++ static/global used by the project as a Rust static, with its
  native name and declaring-header path. This policy does not apply to plugin state.
- Static pointer declarations require an explicit mode:
  - `Value` reads the current pointer by value from the resolved slot on every call.
  - `Address` uses the inline object's address without copying its storage.
- Opt the target struct into `#[runtime_bindings(global)]` before `cpp_struct`.
  Forwarders are inferred from its existing `vtable` declarations. For ordinary
  public inherent methods, also mark their impl `#[runtime_bindings(global)]`.
  Other native types do not generate proxies.
- All proxy methods are unsafe, including forwards to safe Rust methods: native
  storage, lifetime, synchronization, and method contracts remain the caller's duty.
  Receiver/return lifetime relationships are preserved, including borrowed guards.
- Startup calls `initialize_value_bindings(base, offsets)` before access.
- Native globals belong in the module matching their declaring header;
  `GMalloc` is declared in `HAL/MemoryBase.h`.
- Use the native allocator methods directly, for example
  `unsafe { GMalloc.Malloc(size, alignment) }` or `unsafe { GUObjectArray.objects() }`.
  `unsafe { GMalloc.as_ptr() }` supplies a raw pointer for APIs that require one;
  do not retain it across native allocator replacement.
- Binding caches the slot address, not its current contents; binding before
  the engine populates a pointer is permitted.
- Missing/invalid bindings and null pointer values panic with the resolver name.
  Zero scalar values remain valid. Access never returns `Option` or `Result`.
- The caller must uphold native layout, lifetime and synchronization requirements.
- The generated proxy owns no native object, exposes no `Deref`/`DerefMut`, and
  never borrows UE's mutable pointer slot. Resolved addresses use the existing
  atomic registry, without locks. Ordinary native pointer writes must not race
  with slot reads; publishing the address atomically does not synchronize C++ writes.
- Legacy zero-argument unsafe accessors remain supported for generic `Copy`
  values (`Value`) and raw inline addresses (`Address`). Scalars are not given
  implicit arithmetic or object-method forwarding.

## Direct method bindings

Keep the resolver declaration separate, then bind a method to its type:

```rust,ignore
#[unchained_macros::runtime_bindings]
impl FText {
    /// Safety: self owns a live native shared reference, released exactly once.
    #[unchained_macros::bind_resolved_func(FText__Destructor)]
    pub unsafe fn Destructor(self: *mut Self) {}
}
```

- Supports `&self`, `&mut self`, `self: *const Self`, `self: *mut Self`, and
  static/free functions. Raw receivers require the plugin's nightly feature.
- Preserves visibility, attributes, parameter order and return type. Uses the
  existing Windows x64 `extern "C"` native ABI; signatures must be ABI-correct.
- Requires an empty `unsafe fn` body. Lifetime parameters are supported;
  type/const-generic, async, const and explicit-ABI
  declarations are rejected. Hidden C++ parameters must be declared explicitly;
  the attribute does not infer native layouts or return-storage conventions.
- Registers a native binding, not a replacement. Calls use the resolved entry
  until a detached detour attaches, then use its original-function trampoline.
  Calling the binding from its replacement bypasses that replacement.
- The plugin initializes registrations from cached/scanned resolver RVAs before
  applying patches or hooks. Missing bindings fail startup; premature calls panic.
- Resolver arguments are type paths, so existing complex resolver definitions
  can be reused without embedding pattern or pipeline syntax in this attribute.
  The final identifier is the resolver's cache key; use its declared name, not
  an import alias.

`unchained-macros-impl` implements the attribute: Rust requires procedural macros
to live in a separate crate. Runtime binding state stays in `unchained-macros`.

## Lifetime-aware hooks

Detours keep a `RawDetour` in static `OnceLock` storage. Both entry and replacement
are checked against the higher-ranked native signature before pointer erasure.
Original calls use the native binding. Preserve borrow relationships in both
the binding and replacement:

```rust,ignore
#[unchained_macros::bind_func_detour(Native::Read, INACTIVE)]
unsafe fn read<'a>(value: &'a Native) -> &'a Native {
    unsafe { value.Read() }
}
```

Native validity, aliasing, thread affinity and patch synchronization remain
unsafe obligations. Keep raw pointers where reference guarantees are unknown.

A detour may explicitly override a parameter's outer pointer/reference mutability:

```rust,ignore
#[unchained_macros::bind_func_detour(FUDPPing::UDPEcho)]
unsafe fn ping(
    #[mutability_override] endpoint: *mut FString,
    timeout: f32,
    callback: *mut FIcmpEchoResultCallback,
) {
    unsafe { FUDPPing::UDPEcho(endpoint, timeout, callback) };
}
```

`#[mutability_override]` accepts raw pointers or references. The adapter checks the
actual native type from binding metadata; either direction and exact matches
are allowed. Only the annotated outer mutability is adapted. Pointees, nested
pointer mutability, remaining parameters, returns, and lifetimes remain exact.
Raw pointers and references cannot be interchanged. Every intercepted call must
satisfy the detour's validity, aliasing, and write-access contract. In particular,
`&mut T` requires exclusive access for its entire borrow even when the native
declaration uses `&T`. The annotation does not make shared or read-only memory
writable; proving these guarantees is the unsafe detour author's obligation.
Spell outer pointer/reference types directly in both signatures, not via aliases.

Run `cargo test -p unchained-macros -p unchained-macros-impl` for declaration
validation and direct-call fixtures. Windows hook fixtures also attach real
detours and verify native-entry interception and original-call bypass.
