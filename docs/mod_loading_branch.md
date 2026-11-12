# Mod Asset Registry loading (plugin side)

This plugin discovers mounted mod `.pak` content and registers it into the
runtime **Asset Registry**, so maps, mod markers and loading screens are found
without a game restart. It merges a packaged `ModAssetRegistry.bin` when one is
present and falls back to scanning pak headers otherwise.

**What the markers and the registry file are — and how mod authors produce
them — is documented in the ArgonSDK repository** (the source of truth):
`docs/mod-markers-and-asset-registry.md` in
[Chiv2-Community/ArgonSDK](https://github.com/Chiv2-Community/ArgonSDK), covering
`DA_ModMarker_V2`, `DA_MapMarker_V2` and `ModAssetRegistry.bin`. Packaging
commands live in the
[ArgonSDKTooling](https://github.com/Knutschbert/ArgonSDKTooling) README.

## Implementation notes (this plugin)

- **Discovery** reads pak indexes with `repak` (pure Rust) and parses the
  unversioned cooked UE 4.25 package headers to find marker/map packages without
  touching asset payloads.
- **Registration** runs on the game thread via `ScanPathsAndFilesSynchronous`,
  chunked (`--mod-map-batch`), hidden inside the loading screen.
- **Markers** are read by property name (`DA_ModMarker_V2`, `DA_MapMarker_V2`),
  with the deprecated V1 `DA_ModMarker` still recognised. Map-marker rows
  (excluding `Held`) drive the loading screens; the title/subtitle come from the
  map's Asset Registry map-summary tags.
- **Registry fast path**: a pak that ships `ModAssetRegistry.bin` is merged
  directly and skips the header scan; a missing or rejected registry falls back
  to header discovery for that pak.
- **Timings / profiling**: see `docs/mod_loading_profiling.md`.
