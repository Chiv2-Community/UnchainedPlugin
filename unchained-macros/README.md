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

## Direct method bindings

Keep the resolver declaration separate, then bind a method to its type:

```rust,ignore
impl FText {
    /// Safety: self owns a live native shared reference, released exactly once.
    #[unchained_macros::bind_resolver_func(FText_Release)]
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
- Registers a direct address, not a detour or trampoline. Do not bind a target
  that is also actively intercepted; retain `CALL_ORIGINAL!` for that case.
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
