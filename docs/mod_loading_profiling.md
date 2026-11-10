# mod_loading profiling overview

Startup Asset Registry registration profiling for the mod-map/marker feature
(auto init path, plus the since-removed `registermodmaps` / `loadallassets`
commands that produced the all-assets runs below). Captures run-by-run
timings so we can compare later (e.g. after the partial-read and
parallelization tickets land).

Environment: `--mod-map-batch 128`, shipping game (Chivalry 2, UE 4.25 cooked
unversioned), the mod `Content/Paks` set (35 paks; ~11.3k `.uasset`,
4816 unique package assets).

## Runs

| Run | Date | Path | Result | index / gathering | marker-detection | AR registration | total |
|-----|------|------|--------|-------------------|------------------|-----------------|-------|
| 1 | 2026-11-06 | all-assets, **async** (tick-drained) | 4816 assets | gathering 54.8ms | — | 6.71s* | 6.77s |
| 2 | 2026-11-06 | all-assets, **sync** (during init) | 4816 assets | gathering 43.4ms | — | 4.15s | 4.20s |
| 3 | 2026-11-06 | filtered (maps+markers), sync | 57 pkg (33 markers) | index 1.8ms | 2.09s† | 177ms | 2.39s |
| 4 | 2026-11-06 | filtered, sync (breakdown) | 57 pkg (34 maps, 28 DA_ModMarker, 5 DA_MapInfo) | index 1.9ms | 2.07s† | 179ms | 2.37s |
| 5 | 2026-11-06 | filtered, sync, **parallel (8 threads)** | 57 pkg (34 maps, 28 DA_ModMarker, 5 DA_MapInfo) | — | **390.6ms** | 177.7ms | 569.8ms |

\* Run 1's AR time is inflated — it **includes a ~3s idle gap** while the map
loaded (the old tick-drained registration only ran when `UGameEngineTick`
fired, which stalls during the loading screen). Runs 2–4 run synchronously in
the loading screen, so their AR numbers are clean.

† marker-detection is dominated by **whole-entry `.uasset` read/decompress IO**
(`repak` `reader.get()` decompresses the entire file to reach the header); the
actual header parse is ~63ms across all `.uasset`. This is the target of the
partial-read ticket (`mod_loading-txh`).

**Pak scanning is now parallel** (worker threads, capped at 8; `mod_loading-a55`).
Host measurement on the same paks:

| threads | marker-detection wall |
|---------|------------------------|
| 1 (sequential) | 2.998s |
| 4 | 888ms (~3.4×) |
| 12 | 1.345s (IO-bound; oversubscribed) |

Same 33 marker packages in all three (deterministic). So in-game marker-detection
should drop from ~2.07s toward ~0.9–1.3s.

## Notes / observations

- **Registration set reduction:** filtered path registers **57 required packages**
  (maps + DA_ModMarker/DA_MapInfo) vs **4816** for all-assets — ~80× fewer, into
  a single 57-file batch (~179ms AR).
- **AR cost is linear** in packages handed to the engine (~0.86ms/pkg, game thread).
  Batch size spreads it over frames but doesn't reduce total engine work.
- **Pak scan is now parallel** (worker threads, capped at 8) — see the table above.
- Per-batch times (128/pkg) ranged ~20–420ms in the all-assets runs; heavy map
  packages dominate (CMW_Arena/Cistern).

## Tickets

- `mod_loading-8wa` (epic) — cheaply register only maps + DA_ModMarker/DA_MapInfo.
- `mod_loading-txh` — header-only (partial) pak reads to cut marker-detection (still open).
- `mod_loading-a55` — parallel pak scanning (implemented; capped at 8 threads).
