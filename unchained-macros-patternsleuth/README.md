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
- `define_offset_resolver!(Key)` declares a typed resolver for an RVA supplied
  by an offset-map provider. Bind it with core's ordinary `bind_resolved_func`.

Batch scans before initializing bindings; do not launch a separate scan per method.
Persist and validate the complete selector cache identity. UObject/runtime resolver
addresses are not requirements for this backend's scan cache.

Import binding, detour, patch, and selector APIs directly from core. This crate
exports only backend-specific scanning and offset-provider APIs.
