# PR47 client/server startup profile

Measured 2026-11-21. The branch removes main's extra warm global-resolution
pass, but its resolver-cache-cold scan is slower. These are whole-revision
comparisons, not an isolated measurement of static-global forwarding.

This live-game baseline supplements the older, standalone workloads in
[pr47-performance-review.md](pr47-performance-review.md).

## Revisions and method

- Branch: `3050a5b3904448d8cfbede8c7307cd9a3c4a81c7`.
- Latest main and latest stable 1.x tag, `v1.0.1`, both:
  `2382eee09f11f19b6794e88d73b1e9c10a873945`. They are not independent source
  revisions. The unchanged [published DLL][release] was measured separately.
- Source builds: nightly `2026-10-30`, cargo-xwin `0.23.1`,
  `x86_64-pc-windows-msvc`, `--locked --release --lib`, default features,
  two build jobs. Repository release settings: opt-level 1, full debug info,
  panic unwind, no LTO. No compatibility patches.
- Host: Intel Core Ultra 9 285H, Linux `7.3.4-1-cachyos`, CPUs 0–15 available,
  Proton `experimental-11.0-20261112-x86_64`. CPU allocation was not exclusive.
- Separate copied prefixes, game overlays, profiles, DLL registries and resolver
  caches. Sequential launches; no concurrent benchmark runs or builds.
  Installed game files and normal profiles were not modified.
- Client: 960×540 window, no sound. Server: `FFA_Courtyard`, loopback ports,
  local unavailable browser backend, no backend ban list. Stop after scan-cache
  save or plugin initialization; no gameplay-throughput measurement.
- Source-only timing patches use `Instant` around preinit, patch application,
  hook attachment, worker initialization, scan and `resolve_many`.
  Individual hook callbacks also emit timings. Logging uses debug level and
  microsecond timestamps. Logging and instrumentation overhead remain included.
- Cold means missing **resolver cache**, not flushed OS caches. Warm means a
  completed scan cache. Times are wall-clock phases, not launch-to-playable time.
  Do not add phase medians: cold scan and warm initialization are separate runs,
  and individual phase distributions need not align.

## Warm source-build timings

Milliseconds, median [minimum–maximum], three successful unsampled runs per cell.

| Phase | Branch client | Main client | Branch server | Main server |
| --- | ---: | ---: | ---: | ---: |
| Apply patches | 1.70 [1.18–2.43] | 1.27 [1.17–1.67] | 2.70 [2.22–2.75] | 2.69 [2.60–4.45] |
| Attach hooks | 21.11 [14.55–25.95] | 26.79 [21.23–27.80] | 26.28 [14.14–28.20] | 32.50 [28.46–37.70] |
| Preinit, includes patches/hooks | 62.02 [39.94–69.02] | 47.79 [43.81–64.65] | 70.68 [40.43–73.48] | 76.71 [75.58–77.01] |
| Worker `init_rustlib` | 1.96 [1.63–2.26] | 263.09 [253.44–283.71] | 2.22 [1.78–4.63] | 276.62 [252.24–306.27] |

The branch removes main's extra live global-resolution pass in worker
initialization. Hook medians are lower, but ranges overlap substantially;
client preinit's median is higher. No uniform startup speedup is established.
The hook workloads differ: 41 branch callbacks versus 64 main callbacks,
including passive hooks. All callbacks succeeded in the measured warm runs.
The slowest individual callback was the branch's `UObject__ProcessEvent`
(1.52–2.49 ms) or main's `ATBLCharacter__OnDamageTaken` (1.12–2.46 ms).
Callback timings exclude surrounding loop logging; their sum is not the total
hook-attachment phase.

## Resolver-cache-cold timings

Milliseconds, median [minimum–maximum], two successful unsampled runs per cell.

| Phase | Branch client | Main client | Branch server | Main server |
| --- | ---: | ---: | ---: | ---: |
| `resolve_many` | 1039.98 [1019.18–1060.79] | 577.54 [562.02–593.06] | 974.12 [947.88–1000.36] | 611.94 [608.54–615.34] |
| Complete scan | 1090.40 [1066.59–1114.20] | 634.19 [626.31–642.07] | 1028.28 [1000.69–1055.88] | 656.34 [651.63–661.05] |

Branch complete-scan medians are 72% higher on client and 57% higher on server.
The workloads differ: branch requests 108 resolvers; main/release request 105.
Neither those three extra resolvers nor static-global forwarding alone has
been established as the cause. This is a measured regression requiring separate
investigation, not offset by a claim about warm initialization.

An incomplete first main-client scan was stopped by an early harness bug:
the launcher exits before the client game. The corrected harness follows the
game log. That attempt and all CPU-sampled runs are excluded from timing
medians. Every retained cold run saved its resolver cache.

## Published v1.0.1: approximate log windows

Unchanged release DLL; no internal timers. Milliseconds, median
[minimum–maximum], two successful unsampled warm runs per role.
These windows differ from the exact phases above and must not be merged.

| Log window | Client | Server |
| --- | ---: | ---: |
| “Attaching hooks” → “Applied patches and hooks in preinit” | 22.76 [18.40–27.12] | 18.89 [14.19–23.59] |
| “Running resolvers” → “Unchained Sleuth initialized” | 308.63 [300.91–316.34] | 295.21 [283.25–307.17] |
| Cold “Scanning for missing signatures” → “Skipping patch application during scan” | 682.22 [620.78–743.67] | 665.63 [635.66–695.60] |

The cold window includes scanning and cache-update work, not just
`resolve_many`; two successful unsampled cold runs per role.

## CPU sampling limitations

One additional cold server run per variant used
`perf record -e cpu-clock:u -F 49`, inherited across launcher descendants.
All three saved their scan caches. Branch/main/published recordings contain
590/528/563 samples with no reported lost samples. They include .NET launcher,
Wine initialization, game and shutdown work, not a scan-only interval.

Call-chain/register recording is denied by host permissions. Perf labels much
of the game execution `[JIT]`, without usable native function attribution.
These leaf-IP recordings do not identify a resolver hotspot or support a
scan CPU-time comparison. Client comparisons use the phase timers/log windows,
not CPU sampling.

## Evidence

[Portable run data](pr47-startup-profile.json) records per-run timings,
milestones, callback counts and sampling exclusions. Raw logs, measurement-only
patches, scripts, manifests and perf recordings remain in the measurement
host's `/home/jbarber/.cache/opencode/pr47-static-global-profile/`.
That local directory is not a portable download; the checked-in data and tables
do not depend on it being available.

## Artifact identity

SHA-256:

| Artifact | Hash |
| --- | --- |
| Uninstrumented branch DLL, built but not used for timed comparisons | `356359a0de491a3f0f4b759a6acbe10b2afe5e94d34ca66fd502a7f03828a848` |
| Timed branch DLL | `8f50b49e84a4b8e66bef7326b739833f6163574f84efafc551decb08f5445f31` |
| Timed main/tag source DLL | `bd6a77ad6f36a38a8f59df7f18d7956bdd499e62d1cc5a768aef3065dae6e8ac` |
| Published v1.0.1 DLL | `85cf655d7af84ea9d933c06d05efdb11743ab1d5a93aefeff1e0de474ced469d` |
| Steam game executable | `d82b5eaa122aaee15366499bbce34e325741fecd06d6b7b5490fc650b5afb48b` |
| Local validation launcher | `fce2c00cfb1bd04530796552eae2a7bcfebbb300f7e120e02e837296138e2085` |

The launcher is a local validation build with native-console handling and
registry redirects, not an asserted bit-for-bit published launcher.
The source DLLs are Linux cross-builds, not bit-for-bit Windows CI reproductions.

## Repeating the comparison

1. Check out the pinned branch and baseline revisions into separate worktrees.
   Build both with the same pinned toolchain, target, features and release profile.
2. Apply equivalent phase-only timers described above in those isolated trees.
   Keep published v1.0.1 unchanged and verify its hash before launch.
3. Use the same game executable and launcher, separate prefixes/overlays per
   variant and role, and identical role-specific arguments and logging.
4. Archive only each isolated profile's resolver-cache file for cold runs.
   Keep the generated cache for warm runs. Do not flush OS caches.
5. Launch sequentially. Retain two successful unsampled cold runs and three
   warm runs per source variant/role; retain two of each for the published DLL.
   Verify cache-save/initialized milestones, not launcher exit status alone.
6. Summarize phase medians and ranges separately from published log windows.
   Exclude failed and sampled attempts. Compare resolver and hook workloads
   before attributing any difference to one implementation change.

[release]: https://github.com/Chiv2-Community/UnchainedPlugin/releases/tag/v1.0.1
