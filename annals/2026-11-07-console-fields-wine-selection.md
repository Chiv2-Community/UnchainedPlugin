# Console field colors and Wine selection defaults

- Date: 2026-11-07 local (2026-11-08 UTC).
- Status: accepted layout and implemented selection changes; uncommitted.
- Scope: default console presentation and plugin-owned Wine console selection.
- Repository: [UnchainedPlugin](https://github.com/Chiv2-Community/UnchainedPlugin.git); branch `log-standardization-finalization`; recording HEAD `848a4c885c2683066fcdf6e0c67ee46cd00db60b`; no exact tag; nearest tag `v1.0.0-RC3`. Baseline implementation: `3cfc8c1db58323970a39207401949434c06c50f6`, [PR #30](https://github.com/Chiv2-Community/UnchainedPlugin/pull/30).
- Source: user and build agent, session `ses_e5658faf6ffewA56PWUxX9QcPT` through response `msg_1b984e3a9001onDJDIcskuqgK1`, available here as a checkpoint summary; earlier history is compacted. Seven implementation and documentation files have uncommitted changes at recording.

## Decision and context

The user selected an unpadded console layout with gray millisecond timestamps, severity-colored levels, muted banner-gold modules, neutral function and message text, and ` - ` before the message. The working-tree YAML and built-in pattern implement it. The level remains right-aligned; the module abbreviation retains its 32-character cap. This replaces the proposed severity-colored combined header in the [preview](2026-11-07-console-format-preview.md), and changes the fixed-width, bracketed default described in [logging standardization](2026-11-07-console-logging-standardization.md), but does not change the committed baseline until the work is committed. Fixed-width alignment was dropped; the detailed reason for the final color choices is unavailable.

The earlier [redirected-input fix](2026-11-07-console-selection-input-buffer.md) set QuickEdit through `CONIN$`, but mode bits alone did not establish that Wine's renderer would select text. The working tree additionally installs temporary app-specific QuickEdit registry defaults before console allocation under Wine and synchronizes with `CONOUT$` before restoring defaults. The source code attributes this to Wine caching QuickEdit at renderer creation; this is implementation evidence and the rationale reported by the build agent, not an independently verified Wine internals investigation. The existing `CONIN$` path preserves redirected stdin. No other selection alternatives are established by the available history.

## Evidence, consequences, and limitations

- `src/logging/log4rs/log4rs.default.yaml`, `palette.rs`, and `docs/logging.md` show the new default; `windows.rs` shows the temporary defaults and synchronization. `console.rs` includes a Wine synthetic mouse-drag highlight regression test.
- The external `windows-colored-fields-selection-tests.log` reports 175 passed and 16 ignored under the source-reported Proton run. The external `field-colors-report.txt` reports `mouse_highlight=true`, changed pixel values, and input mode `0x1e7`; the checkpoint identifies this as a synthetic drag in a running game console. The report also contains `attach=1 error=14007`; the meaning of the error field is not established here. Neither artifact is a committed repository artifact or independently pinned to an executable hash in the available checkpoint.
- Synthetic highlighting improves on the previous mode-only observation but does not prove human selection, gameplay responsiveness, native Windows behavior, or file output during selection. It does not establish the root cause of the user's original failure.
- Temporary registry defaults and console creation introduce timing and host-compatibility risks; changes are not committed. The queue capacity and loss/memory tradeoffs in the standardization record remain in force at the committed baseline.

## Follow-up and reconsideration

Review and commit the working-tree changes through the normal PR process if approved; verify real user selection, gameplay and file output under Proton and native Windows. Reconsider the defaults or synchronization if supported console hosts fail, registry restoration is unreliable, selection still fails, or display accessibility and compatibility issues arise. The full source conversation, final runtime binary identity, original failure's cause, and native-Windows results remain unavailable.

## Subsequent developments (2026-11-07)

The neutral function color described above was superseded by gold for the module, `::`, and function; the separator was specified as gray. See [Console function and separator colors](2026-11-07-console-function-separator-colors.md). The remaining selection decision and limitations are not reversed.

The previously uncommitted work was subsequently committed in `2c1649a`; see [implementation milestone](2026-11-07-console-selection-format-implementation.md). Validation limitations remain.

The neutral message-body choice was superseded for debug and trace by matching gray level colors; see [Console debug and trace message-body colors](2026-11-07-console-message-body-colors.md). Other levels retain neutral bodies.
