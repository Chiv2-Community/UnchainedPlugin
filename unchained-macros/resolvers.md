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

`startup_resolvers_for(key)` selects roots from compiled bindings and applicable patches, not
Patternsleuth's entire inventory. Declarations alone are not roots; dependencies
resolve automatically in the shared context.

Add `unchained_macros::require_resolver!(Name)` for required scan side effects,
such as populating request offsets. Request-key bindings are not resolver types.
Project declarations override upstream namesakes. Host selection and cache
filtering still apply; existing unrelated cache entries are retained.

## Macros

`define_pattern_resolver!` accepts:

```rust,ignore
define_pattern_resolver!(Name, ["AA BB CC"]);             // Simple
define_pattern_resolver!(Name, Call, ["E8 | ?? ?? ?? ??"]);
define_pattern_resolver!(Name, select Mode; [current_mode(), Mode::Shared], {
    Mode::Read: ["AA BB"],
    Mode::Write: ["CC DD"],
    Mode::Shared: ["EE FF"],
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
annotated with `resolver_selection_key`. They append matches in the explicit
selection-list order. Missing keys contribute nothing; there is no implicit
shared key or fallback. Duplicate dictionary keys and unknown/mismatched variants
are compile errors. Use `None` to disable a key, including its hooks/patches:

```rust,ignore
define_pattern_resolver!(ReadOnly, select Mode; [current_mode(), Mode::Shared], {
    Mode::Read: ["AA BB"],
    Mode::Write: None,
    Mode::Shared: ["CC DD"],
});
```

The first selected key determines applicability; `None` overrides shared
patterns and prevents inline execution. `[]` means enabled with no local
patterns. Omitted optional keys remain enabled and contribute no patterns.

Declare required keys with
`#[resolver_selection_key(require = [Read, Write])]`. Every dictionary must include those keys
with patterns or `None`; omissions fail at compile time.
The trait exposes them as `SelectionKey::REQUIRED`.
Plain pattern lists are selector-independent and apply to every key.
The enum type after `select` publishes typed resolver availability for
[compile-time patch compatibility checks](patches.md).

For custom processing, supply expressions and an inline body returning
`patternsleuth::resolvers::Result<u64>`. The body receives the combined patterns:

```rust,ignore
define_pattern_resolver!(Name, ["AA BB"], |ctx, patterns| {
    let matches = ctx.scan(
        patternsleuth::scanner::Pattern::new(patterns[0]).unwrap()
    ).await;
    patternsleuth::resolvers::ensure_one(matches)
});
```

The inline-body form also accepts selection dictionaries. Prefer
[`define_custom_resolver!`](README.md#custom-resolvers) for composed
BinaryBloodhound matchers producing exactly one address.

## Runtime support

- [`src/resolvers.rs`](src/resolvers.rs): startup-root selection.
- [`src/signatures.rs`](src/signatures.rs): pattern modes and declarations.
- [`src/custom_resolver.rs`](src/custom_resolver.rs): BinaryBloodhound integration.
- [`src/binding.rs`](src/binding.rs): address publication and native calls.
- [`src/registry.rs`](src/registry.rs): hook, patch, and offset inventories.
- [`src/selection.rs`](src/selection.rs): typed consumer-defined restrictions.
- [`startup.rs`](../src/unchained/startup.rs): host executable-base discovery.

Validate game targets with fresh binary scans, not snapshots asserting production
patterns, addresses, keys, registrations, or signatures. Generic algorithm and
local-fixture tests do not prove a native game's contract.
