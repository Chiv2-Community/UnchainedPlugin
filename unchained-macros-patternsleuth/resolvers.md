# Native resolvers

Resolve a verified target beside its native declaration under `src/Chivalry2/*`.
See [native ownership and paths](../docs/engine-sources.md),
[binding contracts](../docs/native-bindings.md), and the
[hook workflow](../CONTRIBUTING.md#creating-game-hooks).
Do not infer an owner, receiver, or ABI from an SDK filename alone.

Project method keys use `Class__Method`; free functions use `Function`.
Preserve upstream names and native capitalization. Keys persist in address caches:
renaming one requires updating every consumer and a fresh scan.

## Startup roots

`startup_resolvers(&snapshot)` selects roots from compiled bindings and applicable patches, not
Patternsleuth's entire inventory. Declarations alone are not roots; dependencies
resolve automatically in the shared context.

Add `unchained_macros_patternsleuth::require_resolver!(Name)` for required scan side effects,
such as populating provider offsets. Declare those keys with
`define_offset_resolver!(Key)` and bind them through `bind_resolved_func(Key)`.
Offset-only declarations are typed resolvers but do not add scan roots.
Project declarations override upstream namesakes. Host selection and cache
filtering still apply; existing unrelated cache entries are retained.

## Macros

`define_pattern_resolver!` accepts:

```rust,ignore
define_pattern_resolver!(Name, ["AA BB CC"]);             // Simple
define_pattern_resolver!(Name, Call, ["E8 | ?? ?? ?? ??"]);
define_pattern_resolver!(Name, {
    Mode::Read: ["AA BB"],
    Mode::Write: ["CC DD"],
    Mode::Other: ["EE FF"],
});
```

These are syntax examples, not game signatures.

| Mode | Selection |
| --- | --- |
| `Simple` (default) | One unique address across all supplied string patterns |
| `First` | First match in supplied-pattern order; only use with verified ordering |
| `Call` | One unique rel32 target; place `\|` at the displacement |
| `XrefFirst` / `XrefLast` | First/last containing root function for references to the first supplied `Pattern` |

Xref modes ignore later patterns. Verify reference ordering; they do not require
a unique function candidate.

Selection dictionaries take a consumer-defined, non-generic fieldless enum
annotated with `resolver_selection_key`, plus a `ResolverSelector` implementation
that supplies `current()`. Qualified keys identify that provider.
Use `fallback = Other` on the attribute to generate a terminal fallback, or
override `fallback(self)` for longer chains. The snapshot freezes the chain and
rejects cycles. Duplicate keys, empty lists, and unknown/mismatched variants are
compile errors. Use `None` to disable a key, including its hooks/patches:

```rust,ignore
define_pattern_resolver!(ReadOnly, {
    Mode::Read: ["AA BB"],
    Mode::Write: None,
    Mode::Other: ["CC DD"],
});
```

Only a missing dictionary key advances to fallback. The first declared entry
wins; `None` stops resolution and prevents inline execution. A declared list
that fails to match the binary does not fall back. Exhausting the chain disables
the resolver for that selection.

Declare required keys with
`#[resolver_selection_key(require = [Read, Write], fallback = Other)]`.
Every dictionary must include required keys with nonempty patterns or `None`;
omissions fail at compile time, even if a fallback exists.
The trait exposes them as `SelectionKey::REQUIRED`.
Plain pattern lists are selector-independent and apply to every key.
The qualified enum keys publish typed resolver availability for
[compile-time patch compatibility checks](patches.md).

For custom processing, supply expressions and an inline body returning
`patternsleuth::resolvers::Result<u64>`. The body receives the chosen entry's patterns:

```rust,ignore
define_pattern_resolver!(Name, ["AA BB"], |ctx, patterns| {
    let matches = ctx.scan(
        patternsleuth::scanner::Pattern::new(patterns[0]).unwrap()
    ).await;
    patternsleuth::resolvers::ensure_one(matches)
});
```

The inline-body form also accepts selection dictionaries. Prefer
[`define_custom_resolver!`](#custom-resolvers) for composed
BinaryBloodhound matchers producing exactly one address.

## Custom resolvers

The matcher must take no input and produce exactly one `u64` address:

```rust
use binary_bloodhound::BinaryMatcher;
unchained_macros_patternsleuth::define_custom_resolver!(Example,
    BinaryMatcher::address(1).ensure_one());
```

Use `using SelectorType, |key| ...` to read the frozen context selection.
Use `delegate upstream::Resolver` to adapt an existing Patternsleuth resolver.
Both forms retain transitive dependencies in the shared scan context; neither
requires an independent scan.

## Runtime support

- [`src/backend.rs`](src/backend.rs): startup roots, scan context, and offset providers.
- [`src/signatures.rs`](src/signatures.rs): pattern modes and declarations.
- [`src/custom_resolver.rs`](src/custom_resolver.rs): BinaryBloodhound integration.
- [`core binding`](../unchained-macros-core/src/binding.rs): address publication and native calls.
- [`core registry`](../unchained-macros-core/src/registry.rs): typed hook and patch inventories.
- [`core selection`](../unchained-macros-core/src/selection.rs): consumer-defined restrictions.
- [`startup.rs`](../src/unchained/startup.rs): host executable-base discovery.

Validate game targets with fresh binary scans, not snapshots asserting production
patterns, addresses, keys, registrations, or signatures. Generic algorithm and
local-fixture tests do not prove a native game's contract.
