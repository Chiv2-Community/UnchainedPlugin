# unchained-macros-patternsleuth

Patternsleuth backend for `unchained-macros-core`.

- `PatternsleuthResolver` resolves a named scan result or provider-offset key.
- `ScanContext` contains the module base and resolved relative offsets.
- `define_pattern_resolver!` and `define_custom_resolver!` generate
  `Resolver<PatternsleuthResolver>` descriptors.
- `startup_resolvers` selects consumed scan roots, excluding other backend types.
- `SelectorSnapshotExt` seeds a frozen core snapshot into a shared Patternsleuth
  context. Import the trait to call `snapshot.eval` or `snapshot.resolve_many`.
- `require_resolver!` registers scan side effects/offset-map providers.
- `bind_resolved_request_function` adapts `GetPost_*` provider keys without
  introducing request-specific code into core.

Batch scans before initializing bindings; do not launch a separate scan per method.
Persist and validate the complete selector cache identity. UObject/runtime resolver
addresses are not requirements for this backend's scan cache.

The compatibility `runtime_bindings` attribute forwards ordinary declarations to
core and translates provider-offset methods into typed resolver declarations.
The `unchained-macros` facade re-exports this adapter and existing scan APIs.
