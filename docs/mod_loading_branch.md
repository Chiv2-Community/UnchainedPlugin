# Branch: `asset-registry-loading` — what it implements

Overview of the mod Asset Registry feature on this branch of `UnchainedPlugin`,
with a focus on what is **parallelized / optimized** today (and what is not yet).

Related docs: `docs/mod_loading_profiling.md` (timings).

## What the branch does

Discovers mod content from the loader's dynamically-mounted `.pak` files under
`Content/Paks` and registers it into the runtime **Asset Registry**, so in-game /
main-menu queries (`GetAssetsByPath`, `GetAssetsByClass`) can find mod **maps**
(`World`) and **marker data assets** (`DA_ModMarker`, `DA_MapInfo`) without a
reboot. This is the `mod_loading-5r1` epic + the `mod_loading-8wa` filtered
epic.

### Marker classes (V1 and V2)

Branch `mod-marker-v2` adds the V2 markers next to the deprecated V1 one:

| Class | Header scan (registered into the AR) | Read at runtime |
|-------|--------------------------------------|-----------------|
| `DA_ModMarker_C` (V1, deprecated) | yes | `ModManager`: hard-ref `TMap` at fixed offset 0x30 |
| `DA_ModMarker_V2_C` | yes | `ModManager`: `ModActorClasses` soft class paths, each class loaded for its CDO |
| `DA_MapInfo_C` | yes | no |
| `DA_MapMarker_V2_C` | yes | `features/map_markers.rs`: `ShippedMapPaths` + `DefaultLoadingScreen` cached per map, used by `resolvers/loading_screens.rs` |

- V2 fields are located **by property name** on the live class
  (`ue::find_property`, a plain walk of `UStruct::child_properties`, no extra
  signatures). `game/unchained.rs::DA_ModMarker_V2_C` mirrors the dumped layout
  and is the fallback when a property is not found; a warning is logged when the
  live offset differs from the dump or the fallback is used.
- `Mod` entries are the same for both versions (name/author/version from the
  mod actor CDO). A mod actor listed by a V1 and a V2 marker yields one entry,
  taken from V2.
- Map markers are read once, as a game-thread job queued after registration
  (`scan_map_markers`). Each shipped map gets its own loading screen texture or,
  if it has none, the marker's `DefaultLoadingScreen`; `GetDataForMap` serves a
  cached map before the built-in table. The marker has no text fields, so the
  map name is used for the entry's first text and the second stays empty.
- `BundledMarkers` are not followed: bundled markers are separate assets and
  come back from the same registry query. A bundled path missing from the
  registry is logged.

### Entry points / commands
| Command | Mode | What it registers |
|---------|------|-------------------|
| `registermodmaps` | async, chunked | maps only (`.umap` under `/Game/Mods/**`) |
| `loadallassets` | async, chunked | every pak entry (benchmark/diagnostic) |
| `listmodmaps` | diag (game thread) | lists registered `World`/assets |
| auto (at `LoadFrontEndMap`, one-shot) | **sync, filtered** | maps + `DA_ModMarker`/`DA_MapInfo` packages (the production init path) |

## Architecture (3 layers)

1. **Discovery** — `repak` reads pak indexes (names/offsets/sizes). Pure Rust,
   no engine calls.
2. **Marker detection** — `src/features/cooked_header.rs`: hand-rolled parser of
   the game's *unversioned cooked* UE 4.25 package header (summary +
   name/import/export tables). No crate dependency.
3. **Registration** — `ScanPathsAndFilesSynchronous` on the game thread, chunked
   (`--mod-map-batch`, default 128).

## What is already optimized

- **Off-thread discovery** — the `*_async` command paths run the pak scan on a
  tokio `spawn_blocking` worker, so gathering never touches the game thread.
- **Header-only marker detection** — only the package header tables are read to
  resolve each export's class; the object *payloads* are never touched. This is
  what lets the filtered path detect `DA_ModMarker`/`DA_MapInfo` instances cheaply.
- **Filter to the needed set** — the production/auto path registers only
  **maps + marker packages** (57 for the test profile) instead of all 4816
  assets → AR registration drops to ~179ms.
- **Dedup by engine package path** — a package present in several mounted paks
  is registered once.
- **Sync registration during init** — the auto path runs all batches back-to-back
  inside the loading screen (hidden cost, no in-game hitch), instead of one
  batch per tick after the map loads.
- **Configurable batch size** (`--mod-map-batch`) + per-batch timing.
- **Signature hardening** — relocation-sensitive bytes wildcarded in the asset
  registry patterns so they survive game rebuilds.
- **Correct engine `this`** — passes the impl object pointer (not the interface)
  to the non-virtual `ScanPathsAndFilesSynchronous`; registration deadlock fixed
  (tokio `oneshot` instead of blocking `mpsc::recv`); commands client-available.

## Parallelism — current state

**What is parallelized:**
- Discovery (pak scan) is off the main/game thread (async `spawn_blocking` for the
  command paths; inline on the game thread only during the init loading screen).
- **Pak scanning is now parallel across worker threads** (pure Rust: repak index
  reads + header parse). `scan_filtered_assets` uses `std::thread` over the pak
  list (strided, deterministic order), capped at 8 threads. Measured on the test
  host: sequential 3.0s → 4 threads 0.89s → 12 threads 1.35s (IO-bound; 8 cap
  avoids oversubscription).

**What is NOT parallelized (yet):**
- Registration is intentionally sequential on the game thread (it drives the
  game-thread-only `ScanPathsAndFilesSynchronous` engine API — cannot be
  parallelized).

### Known bottlenecks / follow-ups
- Whole-entry `.uasset` read still decompresses each file to reach the header
  (~2s sequential, ~0.9s at 4 threads). A header-only partial read is the
  remaining lever → ticket `mod_loading-txh`.

## Why this matters (quick numbers)
Without optimization: register all 4816 assets ≈ 4.2s of asset-registry work on
the game thread. Filtered path: **57 packages**, AR ≈ 179ms, hidden in the
loading screen. Remaining marker-detection IO (~2s) is the next big lever.
