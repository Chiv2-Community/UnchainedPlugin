
# Chiv 2 Resolvers

## Macros
The `define_pattern_resolver` macro eliminates overhead when defining resolvers.

Without the macro, a resolver that scans for a certain pattern and returns a single address could look like this:

```rust
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FString_AppendChars(pub usize);
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
- [macros.rs](./macros.rs)
    - scan functions implementation
    - `define_pattern_resolver`

### Notes
- platforms atm can be STEAM or EGS (read from cli arguments)