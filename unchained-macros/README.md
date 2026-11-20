# unchained-macros

Owns generic hook/detour generation, original-call helpers, memory-patch
registration and signature-resolver macros.

Runtime types and macro invocations stay in the plugin. These macros refer to
the consuming plugin's private modules through `crate::`; references to other
macros use `$crate::` to reach this library. Feature checks remain at the call
site. The plugin imports and re-exports these macros from its crate root.

Logging, rich text, gameplay-specific macros and the procedural attributes in
`sleuth_macros` remain unchanged. Plugin tests exercise generated hook
signatures and lifetime-aware attachment against local fixture functions,
not game functions.

## Custom resolvers

Place the resolver beside its native binding:

```rust,ignore
use binary_bloodhound::BinaryMatcher;
use unchained_macros::define_custom_resolver;

define_custom_resolver!(FText_AsCultureInvariantCopy, {
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
        crate::resolvers::PlatformType::STEAM => "AA BB",
        crate::resolvers::PlatformType::EGS => "CC DD",
        _ => return Err(patternsleuth::resolvers::ResolveError::new_msg("Unsupported platform")),
    };
    BinaryMatcher::patterns([pattern])?.ensure_one()
});
```

The body receives `crate::resolvers::current_platform()` and may use `?`.
It still must produce one `u64` address. This selects matcher behavior; it
does not register a platform restriction for hooks or patches.

## Native bindings and detached detours

New production hooks declare a native binding under `#[runtime_bindings]` in
`src/chivalry2/*`, then target it with `#[bind_func_detour(Type::method)]` from
`src/unchained/patches/*`. See
[Creating game hooks](../CONTRIBUTING.md#creating-game-hooks) for resolver choices,
binding declarations, replacement examples, and validation.

## Legacy resolved function detours

The following API remains for compatibility tests, not new production hooks.

```rust,ignore
#[unchained_macros::runtime_bindings]
impl MyType {
    #[bind_resolved_func_detour(MyResolver, ACTIVE)]
    fn foo_detour(&mut self, value: i32) -> i32 {
        unsafe { self.foo(value) + 1 }
    }
}
```

- Registers with the existing hook inventory; the resolver remains separately declared.
- The optional condition defaults to `ACTIVE`. Accepts `ACTIVE`, `INACTIVE`,
  `CALLED`, `DISABLED`, or a noncapturing closure/function returning `bool`.
- Activation requires both the global activation flag and the condition at attachment.
- Detour names must end with `_detour`. Stripping that suffix generates the
  unsafe original method: `foo_detour` generates `foo`, never `foo_original`.
  An existing associated item named `foo` is a compile error.
- The original has the same visibility, arguments and result as the detour.
  It calls the trampoline, even when disabled, and panics before attachment.
- `runtime_bindings` supports non-generic inherent impls, borrowed/raw `Self`
  receivers, associated functions, and explicit per-call lifetimes. Borrowed
  methods preserve receiver-based return lifetime elision.
- Standalone functions use the detour attribute without `runtime_bindings`,
  with the same suffix rule: `foo_detour` generates `foo`. Use explicit
  lifetimes for standalone borrowed results.
- Native ABI is `extern "C"`; signatures must match the resolved native function.
  Native calls must not unwind through the detour.
- The consuming crate supplies `retour`, `inventory`, and the plugin's resolver
  registration types, as with `CREATE_HOOK!`.

Direct empty unsafe stubs use `#[bind_resolved_func(Resolver)]`.

`detour_is_initialized("Resolver")` and `detour_is_enabled("Resolver")` expose
generated detour state without exposing storage. Unknown names return `false`.
Use initialization checks before optional native calls; disabled detours still
provide their original trampoline.

## Required native values

```rust,ignore
#[unchained_macros::bind_resolver_value(patternsleuth::resolvers::unreal::gmalloc::GMalloc, Value)]
unsafe fn native_gmalloc() -> *mut FMalloc {}

// GUObjectArray is an inline object, not a pointer slot.
#[unchained_macros::bind_resolver_value(patternsleuth::resolvers::unreal::guobject_array::GUObjectArray, Address)]
unsafe fn native_guobject_array() -> *const FUObjectArray {}
```

- A zero-argument unsafe accessor requires an explicit mode:
  - `Value` reads a `Copy` value from the resolved slot (GMalloc's pointer).
  - `Address` returns the resolved address as a raw pointer to an inline global,
    without reading or copying the object's storage.
- Startup calls `initialize_value_bindings(base, offsets)` before access.
- Binding caches the slot address, not its current contents; binding before
  the engine populates a pointer is permitted.
- Missing/invalid bindings and null pointer values panic with the resolver name.
  Zero scalar values remain valid. Access never returns `Option` or `Result`.
- The caller must uphold native layout, lifetime and synchronization requirements.

## Direct method bindings

Keep the resolver declaration separate, then bind a method to its type:

```rust,ignore
#[unchained_macros::runtime_bindings]
impl FText {
    /// Safety: self owns a live native shared reference, released exactly once.
    #[unchained_macros::bind_resolved_func(FText_Release)]
    pub unsafe fn release(&mut self) {}
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

Hooks use retour's `lifetime_detour!` with static `OnceLock` storage. To preserve
borrow relationships, supply lifetimes before the full hook configuration:

```rust,ignore
CREATE_HOOK!(Read, for<'a>, ACTIVE, PRE, &'a Value, (value: &'a Value), {});
```

Native validity, aliasing, thread affinity and patch synchronization remain
unsafe obligations. Keep raw pointers where reference guarantees are unknown.

Run `cargo test -p unchained-macros -p unchained-macros-impl` for declaration
validation and direct-call fixtures. The fixtures call real `extern "C"`
functions without mocked vtables or detour attachment.
