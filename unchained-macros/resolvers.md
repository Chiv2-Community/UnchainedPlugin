
# Native resolvers

For the resolver → native binding → plugin detour workflow, see
[Creating game hooks](../CONTRIBUTING.md#creating-game-hooks).

Resolver definitions and native bindings live beside their native owners under
`src/Chivalry2/TBL` and `TBLLoadingScreen` (game modules), `src/Chivalry2/Engine` (UE 4.25-plus modules),
or `src/Chivalry2/Plugins` (native game plugins, including private headers).
Identify original native owner headers/sources, using shipping PDB evidence where
recovered. Generated per-type SDK splits do not establish original header ownership.
Custom blueprint layouts and adapters live under `src/unchained/blueprints`. Startup and
integration patches live under `src/unchained/patches`.
Engine paths mirror native source-category/module/header paths, omitting header
visibility directories. See the [source layout rules](../docs/engine-sources.md).
Plugin replacements live with instruction patches under `src/unchained/patches`, targeting a native
method with `#[bind_func_detour(Type::Method, condition)]`. Organize them by
plugin purpose, using `main`'s resolver feature groups as the basis, not by
native class/header paths. See
[native bindings and plugin detours](../docs/native-bindings.md).
All production detours use this split.
Shared registration, binding initialization, and platform filtering live in
`unchained-macros`; native resolver declarations stay with their owners.

Native source order:

1. Source links and imports.
2. Resolver declarations, separated by blank lines.
3. Each struct followed immediately by one inherent `impl`.
4. Trait implementations and supporting functions/tests.

New native methods use `#[bind_resolved_func]` under `#[runtime_bindings]`;
plugin detours target those methods from outside the native model.
Put additional generic bounds on the methods that need them, not separate impls.
Keep cfg restrictions intact.

- Name project-defined method resolvers `Class__Method` and free functions `Function`; use
  `Owner__Value` or `Value` for globals and `Class__Destructor` for destructors.
- Preserve C++ spelling and `TBL` capitalization.
- Keep upstream resolver type names and intentional integration aliases intact.
- Resolver names are cache keys; update every consumer when explicitly renaming one.

Runtime support:

- [`src/registry.rs`](src/registry.rs): hook, patch, and offset inventories.
- [`src/binding.rs`](src/binding.rs): native binding initialization and state.
- [`src/platforms.rs`](src/platforms.rs): storefront selection and restrictions.
- The plugin discovers its executable base in
  [`startup.rs`](../src/unchained/startup.rs) and passes it to binding initialization,
  hook attachment, and patch callbacks.

- Native methods use `#[runtime_bindings] impl Type` and
  `#[bind_resolved_func(Resolver)] unsafe fn name(...) {}`.
- Model calls invoke the original implementation, bypassing replacements.
- OpenAPI request methods use `#[bind_resolved_request_function(GetPost_Name)]`
  under `#[runtime_bindings]`; their detached replacements target the method
  just like other bindings. Keys come from the request offset registry.
- Native resolvers stay with their owners; `CREATE_PATCH!` belongs in the relevant
  functional module under `src/unchained/patches`.
- Native owners that are not established remain free functions, not guessed receivers.
- Examples: [`UGameEngine`](../src/Chivalry2/Engine/Runtime/Engine/Engine/GameEngine.rs),
  [`UObject::LoadConfig`](../src/Chivalry2/Engine/Runtime/CoreUObject/UObject/Object.rs), and
  [`UNetDriver` patches](../src/unchained/patches/hosting/desync/net_mode.rs).

## Macros
The `define_pattern_resolver` macro eliminates overhead when defining resolvers.

Without the macro, a resolver that scans for a certain pattern and returns a single address could look like this:

```rust
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FString__AppendChars(pub u64);
impl_resolver_singleton!(all, FString__AppendChars, |ctx| async {
    let patterns = [
        "45 85 C0 0F " // Universal
    ];

    let res = join_all(patterns.iter()
        .map(|p| ctx.scan(Pattern::new(p).unwrap()))).await;

    Ok(FString__AppendChars(ensure_one(res.into_iter().flatten())?))
});
```

With the `define_pattern_resolver` macro, the code above can be simplified:
```rust
define_pattern_resolver!(FString__AppendChars, [
    "45 85 C0 0F" // Universal
],
|ctx, patterns| {
    let res = join_all(patterns.iter()
        .map(|p| ctx.scan(Pattern::new(p).unwrap()))).await;
    ensure_one(res.into_iter().flatten())
});
```

Some common scan modes are implemented. We can simplify it more like:


```rust 
define_pattern_resolver!(FString__AppendChars, Simple, [
    "45 85 C0 0F"
]);
```
`Simple` mode is default:

```rust
define_pattern_resolver!(FString__AppendChars, [
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

// Per-platform signatures. OTHER extends the current platform's patterns.
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

// Custom code. patterns combines the current platform and OTHER entries.
define_pattern_resolver!(GetMotd3, {
    EGS: ["4C 89 "],
    STEAM: ["4C 89 "]
},
|ctx, patterns| {
    let futures = join_all(
        patterns.iter()
            .map(|p| ctx.scan(Pattern::new(p).unwrap()))
    ).await;

    ensure_one(futures.into_iter().flatten())
});
```


### Relevant files
- [Resolver macros](src/signatures.rs)
- [Typed signatures](src/signature.rs)
- [Native hook tests](tests/native_hooks.rs)

### Notes
- Runtime platform names are `STEAM`, `EGS`, `XBOX`, and `OTHER`.
