---
name: create-resolver
description: Locate a Chivalry 2 native function in the latest game binary and create or update its address resolver. Use for byte signatures, call targets, string references, platform-specific scans, and custom BinaryBloodhound searches.
---

# Creating a resolver

Find the verified native target, then resolve its address reproducibly.

- Follow [Creating game hooks](../../../CONTRIBUTING.md#creating-game-hooks)
  and the [native source layout](../../../docs/engine-sources.md).
- Read the [resolver reference](../../../unchained-macros/resolvers.md) and nearby
  implementations before choosing a scan strategy.
- This skill locates addresses; binding declarations and plugin behavior are
  separate work.

## Establish the target binary

Analyze the latest available game binary for the requested storefront.

- Identify the executable path, game/build version, storefront, architecture,
  image base, and file hash.
- Confirm it is the latest target build; do not equate an old analysis database
  or symbolized executable with the current release.
- If the latest binary is unavailable or its identity is uncertain, request it
  or report the limitation before treating a result as current.
- Check that an existing analysis database corresponds to that exact binary.
- Match PDB identity when available. Treat older symbols/source as supporting
  evidence, not proof that addresses, signatures, or layouts are unchanged.
- Keep binaries, databases, dumps, and scratch reports outside tracked files.

## Find and verify the function

Use whichever binary-analysis tools are available.

- Discover available integrations or local tools: Ghidra, Binary Ninja, REA,
  IDA, radare2, or another suitable disassembler/decompiler.
- Verify the selected program/database/view before querying addresses.
- Prefer static, read-only analysis. Do not modify running game processes or
  installations as part of resolver discovery.
- Start from relevant strings, imports, exports, symbols, callers, callees,
  vtables, or recognizable behavior.
- Follow references to the implementation; distinguish wrappers, thunks,
  interior instructions, and actual function entries.
- Inspect disassembly as well as decompiler output. Check multiple call sites
  where available; decompiler names and inferred types are not authoritative.
- Establish the native owner and ABI:
  - Static versus instance call and receiver/subobject position.
  - Argument order, widths, pointer use, and hidden return storage.
  - Return convention and any relevant layout or lifetime evidence.
- Record absolute addresses with their image base and corresponding RVAs.
- Do not invent source ownership or ABI details when evidence is incomplete.

## Implement the resolver

Reuse an existing resolver when it already resolves the verified target.

- Search native declarations and existing patternsleuth resolvers first.
- Put new declarations beside the native owner under `src/Chivalry2/*`.
- Follow source-category/header paths; omit `Public`, `Private`, and `Classes`.
- Add new modules to `mod.rs`; put resolvers before types and bindings.
- Name project-defined method resolvers `Class__Method`, matching the target.
  Free-function resolvers use `Function`; globals use `Owner__Value` or `Value`.
  Destructor resolvers use `Class__Destructor`.
- Preserve native capitalization: `TBLFoo`, never `TblFoo`.
- Keep intentional integration aliases and upstream resolver names intact.
- Mirror used C++ statics/globals as Rust static bindings under their declaring
  header, preserving native names. Use `bind_resolved_value(..., Value)` for
  pointer slots or `Address` for inline objects; keep the shared startup registry.
  Resolve/cache the address, never a replaceable pointer slot's contents.
- Resolver identifiers are cache/registration keys. For explicitly requested
  renames, update every consumer; expect fresh scans under new keys.
- If source ownership remains unproven, follow the
  [native ownership guidance](../../../unchained-macros/resolvers.md); do not fabricate
  a native header or receiver.
- Startup roots come from bindings and patches, not every declared resolver.
  Register side-effect/offset-map providers explicitly with
  `unchained_macros::require_resolver!(ResolverName)`. Keep transitive dependencies
  inside the shared Patternsleuth context; do not restore global inventory scans.
- Choose the narrowest verified search:
  - `Simple`: one unique byte-pattern address.
  - `Call`: follow a rel32 target; place `|` at the displacement.
  - `XrefFirst` / `XrefLast`: string references and containing root functions.
    Verify the ordering; these modes only use the first supplied string pattern.
  - `First`: only when first-match selection is independently justified.
  - `define_custom_resolver!`: composed BinaryBloodhound searches producing a
    no-input matcher with exactly one `u64` address.
- Wildcard relocations and build-dependent bytes; retain distinguishing context.
- For callable bindings, return a function entry, not an interior match.
- Platform dictionaries combine current-platform patterns with `OTHER`;
  `OTHER` is not a fallback.
- Use `define_single_platform_pattern_resolver!` for explicit storefront
  restrictions. Platform-aware custom searches alone do not restrict installation.
- Use the [custom resolver examples](../../../unchained-macros/README.md#custom-resolvers)
  for composition and error handling.

Example search shape; replace the illustrative pattern with verified bytes:

```rust
use binary_bloodhound::BinaryMatcher;

define_custom_resolver!(ExampleFunctionEntry, {
    BinaryMatcher::patterns(["48 89 5C 24 ?? 57"])?
        .root_functions()
        .addresses()
        .ensure_one()
});
```

## Validate and hand off

Prove the search identifies the intended entry in the target builds.

- Run a fresh scan against the identified binary; cached RVAs are not validation.
  Use isolated fixtures or scratch outputs rather than deleting user caches.
- Check match counts and inspect the resolved function independently.
- Test supported storefront/build variants; report unavailable variants explicitly.
- Do not add fixtures or snapshots that validate individual game resolver
  patterns, addresses, keys, registrations, or native signatures.
- Keep generic scanner/matcher algorithm tests; they do not establish a game's
  native contract. Validate individual resolvers with fresh binary scans.
- Run the contributor guide's Windows build checks with affected features.
- Report the resolver key, native module path, binary identity, entry/RVA,
  selection evidence, ABI findings, and validation limits.
- Do not claim latest-build or cross-platform support from an older binary alone.
