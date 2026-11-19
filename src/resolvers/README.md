
# Chiv 2 Resolvers

Resolver definitions, detours, and patches live beside their native owners under
`src/chivalry2/tbl` (ArgonSDK TBL files) or `src/chivalry2/engine` (UE 4.25-plus modules).
Game model files each identify one corresponding ArgonSDK source file.
Private/custom engine integrations live under `src/chivalry2/engine/extensions`.
This directory contains only shared registration, platform filtering,
and tests. Keep detour bodies and supporting state with the native implementation;
do not add forwarding handlers here.

Native source order:

1. Source links and imports.
2. Resolver declarations, separated by blank lines.
3. Each struct followed immediately by one inherent `impl`.
4. Trait implementations and supporting functions/tests.

Combine binding and detour methods under the same `#[runtime_bindings]` impl.
Put additional generic bounds on the methods that need them, not separate impls.
Keep cfg restrictions intact.

What remains here:

- `mod.rs`: process image base, function-binding initialization/readiness checks,
  hook/patch/offset inventories, and plugin registration invariants.
- `platforms.rs`: platform restrictions and filtering tests.
- Reusable macros, typed signatures, and native hook tests:
  [`unchained-macros`](../../unchained-macros).

- Methods use `#[runtime_bindings] impl Type` and
  `#[bind_resolved_func_detour(Resolver)] fn name_detour(...)`.
- The generated unsafe `name(...)` calls the trampoline.
- OpenAPI request methods use `#[bind_resolved_request_function(GetPost_Name)]`
  for direct calls or `#[bind_resolved_request_function_detour(GetPost_Name)]`
  for hooks inside `#[runtime_bindings]`. Keys come from the request offset
  registry, not Rust resolver types. Direct bindings require empty `unsafe fn`
  bodies; detours retain the `_detour` suffix and generate unsafe original callers.
- Patch resolvers and `CREATE_PATCH!` stay together in the owner's module.
- Native owners that are not established remain free functions, not guessed receivers.
- Examples: [`UGameEngine`](../chivalry2/engine/engine/ugame_engine.rs),
  [`UObject::LoadConfig`](../chivalry2/engine/object/config.rs), and
  [`UNetDriver` patches](../chivalry2/engine/engine/unet_driver.rs).

## Macros
The `define_pattern_resolver` macro eliminates overhead when defining resolvers.

Without the macro, a resolver that scans for a certain pattern and returns a single address could look like this:

```rust
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FString_AppendChars(pub u64);
impl_resolver_singleton!(all, FString_AppendChars, |ctx| async {
    let patterns = [
        "45 85 C0 0F " // Universal
    ];

    let res = join_all(patterns.iter()
        .map(|p| ctx.scan(Pattern::new(p).unwrap()))).await;

    Ok(FString_AppendChars(ensure_one(res.into_iter().flatten())?))
});
```

With the `derive_pattern_resolver` macro, the code above can be simplified:
```rust
define_pattern_resolver!(FString_AppendChars, [
    "45 85 C0 0F" // Universal
],
|ctx, patterns| {
    let res = join_all(patterns.iter()
        .map(|p| ctx.scan(Pattern::new(p).unwrap()))).await;
    ensure_one(res.into_iter().flatten())?
});
```

Some common scan modes are implemented. We can simplify it more like:


```rust 
define_pattern_resolver!(FString_AppendChars, Simple, [
    "45 85 C0 0F"
]);
```
`Simple` mode is default:

```rust
define_pattern_resolver!(FString_AppendChars, [
    "45 85 C0 0F" 
]);
```

### Supported Syntax
list of Patterns
- `NAME, MODE, [ PATTERNS ]`
- `NAME, [ PATTERNS ], | ctx, patterns | { CODE }`

per-platform dictionary. 
Platforms are (EGS, STEAM, OTHER, XBOX). 
- `NAME, MODE, { PLATFORM: [ PATTERNS ] }`
- `NAME, { PLATFORM: [ PATTERNS ] }, | ctx, patterns | { CODE }`

Patterns can be strings or `Pattern` (e.g. `utf8_pattern`)

Examples:

```rust
// Scan for multiple patterns
define_pattern_resolver!(ApproveLoginTwo, Simple, [
    "48 89", // EGS
    "48 89", // STEAM
]);

// Per-platform signatures. Other extends platform-dependent result
define_pattern_resolver!(GetMotd2, Simple, {
    EGS: ["4C 89"],
    STEAM: ["4C 89 "],
    OTHER: ["BE EF"]
});

// Return function by call
define_pattern_resolver!(SomeFunction, Call, [
    "E8 | ?? ?? ?? ?? BE EF ?8 74 3?", // EGS
]);

// get string xref and return root function
define_pattern_resolver!(ApproveLoginFour, XrefLast, [
    util::utf8_pattern(" Minutes")
]);

// Custom code. patterns contains one signature (depending on platform)
define_pattern_resolver!(GetMotd3, {
    EGS: ["4C 89 "],
    STEAM: ["4C 89 "]
},
|ctx, patterns| {
    let futures = join_all(
        patterns.iter()
            .map(|p| ctx.scan(Pattern::new(p).unwrap()))
    ).await;

    ensure_one(futures.into_iter().flatten())?
});
```


### Relevant files
- [Resolver macros](../../unchained-macros/src/signatures.rs)
- [Typed signatures](../../unchained-macros/src/signature.rs)
- [Native hook tests](../../unchained-macros/tests/native_hooks.rs)

### Notes
- platforms atm can be STEAM or EGS (read from cli arguments)
