# PR47 performance review

Compared `07749c63267e9edf4ffba0a6de44691b227669e9` (`main`) with
`c01e1555573f833abc990736c3cfe4b2a65e8a14` (PR47), on Linux.
Results apply to these commits, not subsequent cleanup.

Subsequent cleanup integrated as `8926985` removes the per-hook offset-map
clones and consolidates detour registrations. `9641a7d` removes redundant typed
detour wrappers. Neither change was included in these timings. The C++ lookup
and globals-batching findings below remain open.

## Resolver measurements

A standalone, optimized host harness loaded the installed Steam PE read-only.
No game process, installation, hooks, or address-cache files were modified.

- PE: 100,177,408 bytes; SHA-256
  `d82b5eaa122aaee15366499bbce34e325741fecd06d6b7b5490fc650b5afb48b`.
- Exact Patternsleuth revisions from each commit's lockfile:
  main `644f980`; head `3de6cfae694a95db619eddb989f0e2a9c0891ac6`.
- Same harness, nightly `2026-10-30`, release profile, two Rayon threads,
  CPU affinity `10,11`; five samples per workload.
- The final run disables the optional function cache, matching Windows
  `process::internal::read_image`. File-backed PE sections still differ from a
  loaded process image; this is not a live startup measurement.
- The six-target comparison alternated old/new order. Each sample constructed a
  fresh image and resolution contexts. File data was already resident; image
  setup was measured separately. The harness lockfile was seeded from head's
  lockfile; common transitive dependency versions were preserved. The old/new
  scanner revisions ran together, not as separately built full plugins.
- Other agents were compiling concurrently. Affinity was not exclusive; these
  are local workload measurements, not controlled whole-game benchmarks.

| Workload | Median | Range |
| --- | ---: | ---: |
| Main: six common globals, combined | 0.780 s | 0.776–0.794 s |
| Head: same six globals, combined | 0.710 s | 0.697–0.721 s |
| Main: 13 targets corresponding to globals collector | 1.008 s | 1.001–1.017 s |
| Head: six globals, production's three-call grouping | 1.184 s | 1.183–1.225 s |

All six/13 targets resolved successfully; common target addresses agreed.
Image setup took roughly 0.3 ms and is excluded above. This reproduces
resolver workloads through `resolve_many`, not the full plugin initialization
or the collector's wrapper overhead.

**Actionable regression:** `src/tools/hook_globals.rs:206–216` resolves
`GMalloc`, then `GUObjectArray`, then the remaining collector in separate calls.
Patternsleuth creates a fresh context per call, losing scan batching and
resolver-result sharing. Despite fewer targets, this workload was slower than
main's collector. Combining head's targets saved about 0.47 s in this harness.
Restore one collector/evaluation; preferably initialize all globals from the
already validated address cache.

## Cached versus uncached startup

- Complete cached offsets bypass `scan::scan`, not all scanning.
  `init_globals` still reads the process image and resolves globals separately
  on both revisions. Main resolves 13 collector members; head resolves six
  targets across three contexts.
- Both revisions also hash the executable to select the cache. That unchanged
  file-read/hash cost is outside the resolver timings.
- The version change from `1.0.0` to `1.0.1` invalidates the old cache
  (`src/lib.rs:103–133`). Many resolver keys also changed. The first upgrade
  launch therefore scans and exits for restart; that is not steady-state cost.
- Uncached resolution still batches registered resolvers through
  `resolve_many`. Source declaration counts are not scan-pass counts:
  dependencies, platform filters, pattern alternatives, xrefs, and stages matter.
- The core scanner remains Patternsleuth's grouped byte-pattern scanner.
  BinaryBloodhound's pattern sources submit `ctx.scan` to that same evaluation
  queue; it is not a second whole-image scanning backend. Matcher chains add
  candidate/provenance allocations and navigation work where used.
- The old/new common workload does not establish full uncached-scan speed.
  No complete before/after resolver registry or live startup timing was measured.

## Hook setup and steady-state calls

Structural analysis, not live timing:

- Direct bindings add static registrations and address initialization; they do
  **not** install detours. Head has 65 direct-binding attributes and 45
  `bind_func_detour` attributes in `src` before feature/platform filtering.
  Declaring native types or verified vtable slots does not itself add scans.
- This removes unnecessary setup for old call-only hooks. For example, main's
  `asset_registry.rs` initializes disabled `CALLED` detours for reader
  construction/destruction, FName construction, object loading, and registry
  queries. Direct bindings/vtable calls need no such trampoline allocation.
  This is a structural setup improvement, not a measured installation speedup.
- `FunctionBinding::address` reads the trampoline atomic, falling back to the
  entry atomic; generated methods call that pointer. There is no scan, string
  lookup, inventory walk, or registry mutex in the ordinary binding call.
- Hook attachment adds duplicate-address reservation and an alias-registration
  walk before enabling each patch. These are one-time startup costs.
- `attach_hooks` still clones the complete offsets map per hook, including
  head's method-targeted attach functions that ignore that argument. Passing a
  borrowed map would remove these allocations. This already existed on main.
- `publish_function_trampolines` walks all direct bindings per hook. Indexing
  aliases by entry would improve scaling, but these registry sizes do not
  justify claiming a major observed startup problem.
- Fallible helpers explicitly using `function_is_bound`,
  `require_function_bindings`, or `detour_is_*` do perform linear inventory
  walks. Keep those checks out of high-frequency loops.
- Gameplay work dominates more plausibly than binding dispatch. Native casts
  validate each superclass with `ReadProcessMemory` and convert class FNames
  to allocated strings. Main also did class-name traversal, but head adds
  per-class liveness validation. Profile combat callbacks before optimizing;
  preserve validation and consider validated class/FName ancestry caching.

No Windows hook-installation, trampoline-execution, frame-time, or gameplay
throughput measurements were made. Linux PE scanning cannot establish those.

## Build-cost findings

Exact-tree build attempts used:

```text
RUSTUP_TOOLCHAIN=nightly-2026-10-30
CARGO_BUILD_JOBS=2
TMPDIR=/home/jbarber/.cache/opencode/pr47-tmp
taskset -c 12,13 cargo build --locked --offline --lib -j 2
taskset -c 12,13 cargo xwin build --locked --offline --lib -j 2 \
  --target x86_64-pc-windows-msvc
```

Each revision/target used a fresh, separate `CARGO_TARGET_DIR`. Cargo's source
and Windows SDK downloads were already cached; OS file caches were not cleared.
The same local source-root environment applied to both revisions.
Tools: rustc `1.101.0-nightly (92b5b8c87)`, cargo-xwin `0.23.1`,
Clang/LLD `23.1.5`.
Failures prevent an exact before/after total-build percentage:

- Main host: pinned `openssl-sys 0.9.112` rejects installed OpenSSL 4.1.
- Head host: Windows-gated `http::client`, `spawn_console`, and `seh` are called
  from code compiled on Linux. This is not a supported host library build.
- Main Windows cross-build: `winres` tries `/bin\x64\rc.exe`, which does not
  exist on Linux. Head's `embed-resource` removes this portability blocker.

These failures are not build-speed measurements. No baseline source or
lockfile was patched to manufacture a successful comparison.

Successful head Windows builds, default features/dev profile:

| Build state | Samples | Median | Range |
| --- | ---: | ---: | ---: |
| Fresh Cargo target cache, dependencies included | 1 | 287.8 s | — |
| No source changes | 3 | 0.718 s | 0.677–0.736 s |
| Touch `src/lib.rs`, dependencies cached | 3 | 89.4 s | 87.6–90.4 s |

Only one fresh-cache attempt was made per revision/target; failures prevent a
paired comparison. Warm head runs were repeated instead. Two warm repetitions
added Cargo `--timings`: no-op runs executed zero build units; source rebuilds
spent 53.51–53.73 s running the build script, then 33.63–36.16 s compiling/linking
the plugin. These are not Rustdoc runs or Rust incremental-codegen measurements.
Concurrent agents and nonexclusive CPU affinity limit precision.

Standalone native-doc generation, compiled in an unoptimized dev profile with
head's locked dependency versions, three runs per configuration:

| Head generator configuration | Median | Range |
| --- | ---: | ---: |
| Default URL roots; no local C++ lookup | 0.348 s | 0.343–0.466 s |
| Local UE/ArgonSDK roots | 52.848 s | 52.776–53.066 s |

Outputs already existed from exploratory runs and were reused; all runs
included unchanged-output checks. Neither measurement includes
compiling the generator, provenance generation, resource compilation, Rustdoc,
or compiling/linking the plugin. CPU affinity was `8,9`, not exclusive.
Local roots were `/home/jbarber/projects/ai/yolo/{UnrealEngine,ArgonSDK}`.
UE HEAD was `2d1aaeede6780b14c3ef97272e8a68c3429d8231`;
ArgonSDK HEAD was `d34d00f7e1d192fac553bd57973e3f866aac1767`.
ArgonSDK was tracked-clean; UE had staged annals files, not C++ changes.
URL roots do not fetch source files over the network.

**Major local-source build cost:** roughly 53 seconds of doc generation can
recur after an ordinary source edit. This is not evidence that plugin Rust
compilation itself became 53 seconds slower.

- Both commits use the same nightly and disable Rust incremental compilation
  in the dev profile. A source rebuild with cached dependencies is not a Rust
  incremental-build benchmark.
- New build dependencies include `git2`/libgit2, tree-sitter C++, `syn`, and the
  native-doc generator. Additional proc macros and runtime SDK dependencies
  also affect cold compilation. The resolved root closure grew from 393 to
  413 packages across all target/dependency kinds; this is not a count of
  crates actually compiled for one target.
- `build/mod.rs` generates native docs on ordinary builds, not just Rustdoc.
  `build/provenance.rs` watches all `src`, so an unrelated Rust edit also
  reruns documentation generation.
- `cpp::find_implementation` recursively enumerates and reads a C++ module for
  every bound method. Build a per-generation module/file index and reuse parsed
  syntax trees; avoid rescanning a module per binding.
- Provenance watches the shared repository's entire `refs` directory. Commits
  on unrelated branches/worktrees can invalidate a no-op build. Track the
  actual HEAD ref instead.
- The repository has no end-to-end performance benchmark. Its dependency's
  scanner microbenchmarks are not a plugin startup or hooking benchmark.

For a valid total-build comparison, rerun both commits on the same idle native
Windows runner, with separate target caches and identical roots/features/jobs.
Do not compare main's failed cross-build with head's successful build.

## Local evidence

Scratch harnesses, logs, metadata and isolated target caches are under
`/home/jbarber/.cache/opencode/pr47-performance-*`.
The baseline is a separate detached worktree; the user's existing main checkout
and unrelated dirty files were not used or modified.
