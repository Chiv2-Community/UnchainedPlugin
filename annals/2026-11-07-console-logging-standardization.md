# Console selection and logging standardization

- Date: 2026-11-07 (local; implementation commit is 2026-11-08 UTC)
- Status: implemented on PR branch; merge not verified
- Scope: Windows plugin-owned console and default log presentation
- Repository: [UnchainedPlugin](https://github.com/Chiv2-Community/UnchainedPlugin.git)
- Recording branch: `log-standardization-finalization`; HEAD and implementation commit: `3cfc8c1db58323970a39207401949434c06c50f6`; parent `4905eaa3aef5ef46c31f6fded116d1999204ec24`. No exact tag; nearest tag `v1.0.0-RC3`. Working tree clean before recording.
- Source: user and build agent, session `ses_e5658faf6ffewA56PWUxX9QcPT` through response `msg_1b939027e001GS3tvXUMXk0FLE`; earlier conversation is compacted. [PR #30](https://github.com/Chiv2-Community/UnchainedPlugin/pull/30) targets main.

## Decision and context

The user wanted selectable console text without freezing the game. QuickEdit can block console writes during selection; the chosen design renders records on the caller, then queues console I/O to a worker for plugin-owned Windows consoles. Attached or redirected stdio remains synchronous. A blocked worker must not make console flush or shutdown wait; this trades guaranteed delivery at shutdown for responsiveness.

An initial 128-record queue was enlarged at the user's request to 128 * 1024 records to reduce drops. The user explicitly accepted the potential memory cost. The implementation caps each rendered record at 64 KiB combined text and style storage and 256 style changes; full or oversized records are dropped from console output. At capacity record storage can approach 8 GiB plus allocation and queue overhead. Above 100 queued records, emit the requested pause warning once through the normal logger; rearm only after the queue falls below 100. The intent is for the warning and normal records to reach file logging while the console is paused. The earlier small queue reduces memory exposure but was replaced, not retained as the chosen capacity.

Default encoders, rather than `fn;` message prefixes, now present captured function metadata: console uses `[{fn}]`, file encoders use `{M}::{fn}`, and levels are right-aligned. Palette formatting handles colors; banner rows gain trailing padding. This keeps message text separate from presentation. The compacted history reports these as user requirements; detailed earlier rationale is unavailable. Removing `fn;` also changes compatibility for callers using that macro syntax and custom encoders that previously received prefixed messages.

## Evidence and limitations

The implementation and tests are present at the pinned commit in `src/logging/log4rs/queued_console.rs`, `src/logging/log4rs/console.rs`, `src/logging/console/windows.rs`, and `docs/logging.md`. The available Proton test log reports 175 passed and 16 ignored, including capacity, threshold hysteresis and selection-mode tests. The source transcript reports a release build and a clean diff check. The stalled-output test exercises a simulated writer, not a live selection in the final DLL. Native Windows behavior, final-DLL live selection, whole-game responsiveness, and true worst-case allocated memory remain unverified.

A prior sink-only Proton benchmark without timestamp formatting measured approximately 1.18-1.40 microseconds queued versus 0.33 microseconds direct per record, with no drops. It preceded the enlarged queue and backlog accounting and excludes terminal I/O; it is not a measurement of the final implementation or native Windows. The preview used an earlier DLL, not the final queue implementation.

The separate resolver and hook record at Git commit `de3098e9887825904ae2216f668d623ab5c1d990` describes PR #27 and its historical choice to omit function context in hook and patch messages. It is not present on this branch and has not been restored. PR #30 starts from main, not PR #27; combining the branches may change how those logs display context and requires reconciliation rather than assuming either PR supersedes the other.

## Reconsideration and open questions

Revisit queue sizing and warning behavior if real workloads show memory pressure, sustained backlog, unacceptable loss, or measurable latency. Verify selection and file output during a live game on native Windows and on Proton; investigate other synchronous output paths before claiming all game threads are protected. Confirm PR merge and compatibility with concurrent resolver changes. The full pre-compaction discussion, native-Windows measurements, and final-DLL live validation are unavailable here.

## Subsequent developments (2026-11-07)

The user subsequently reported selection did not work in a live preview. A targeted uncommitted input-buffer fix and limited mode validation are recorded in [Console selection with redirected input](2026-11-07-console-selection-input-buffer.md). This does not refute the queued-output design or establish the original failure's cause; actual drag selection remains unconfirmed.

Later uncommitted work changes the default format and reports a synthetic selection highlight; see [Console field colors and Wine selection defaults](2026-11-07-console-fields-wine-selection.md). The committed format and queue decision above remain historical baseline facts; human selection remains unverified.
