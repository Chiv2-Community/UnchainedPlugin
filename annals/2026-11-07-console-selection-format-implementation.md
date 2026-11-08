# Console selection and format implementation

- Date: 2026-11-07 local (2026-11-08 UTC).
- Status: implemented on PR branch; merge not verified.
- Scope: console field colors and plugin-owned console selection.
- Repository: [UnchainedPlugin](https://github.com/Chiv2-Community/UnchainedPlugin.git); branch `log-standardization-finalization`; recording HEAD and implementation commit `2c1649a027eadf0962c80567b25fe79bc297a601`; no exact tag; nearest tag `v1.0.0-RC3`. Working tree clean before recording. Local remote-tracking branch `origin/log-standardization-finalization` contains the commit; current remote state was not queried. [PR #30](https://github.com/Chiv2-Community/UnchainedPlugin/pull/30).
- Source: user and build agent, session `ses_e5658faf6ffewA56PWUxX9QcPT` through response `msg_1b996a4cd001a7SMmQArxiyijD`, supplied as a checkpoint; earlier history is partly compacted.

## Implementation and context

Commit `2c1649a` implements the previously uncommitted [field colors and Wine selection](2026-11-07-console-fields-wine-selection.md), including the [revised function and separator colors](2026-11-07-console-function-separator-colors.md) and [direct console input buffer fix](2026-11-07-console-selection-input-buffer.md). It changes the default format recorded in [logging standardization](2026-11-07-console-logging-standardization.md); it does not reverse the queued-output decision. The committed YAML uses gray millisecond timestamps and separators, severity-colored levels, and unpadded gold `#B49A66` module and function context. Windows console code opens `CONIN$` and uses temporary Wine QuickEdit defaults and `CONOUT$` synchronization.

The prior records preserve the options and known rationale. This milestone changes implementation status, not the recorded rationale or the uncertainty about the original selection failure. No further rejected alternatives are established.

## Evidence and follow-up

The external `windows-colored-fields-selection-tests.log` reports 175 passed and 16 ignored; the separate `function-separator-colors-tests.log` reports one focused palette test passed after the color revision. The full suite preceded that revision and neither log independently identifies the final committed binary. The external `field-colors-report.txt` reports a synthetic drag highlight and mode `0x1e7`, with an unexplained `attach=1 error=14007`. It does not prove human selection, native Windows behavior, gameplay responsiveness, or file output during selection. Wine selection is reported as rectangular; line-based selection remains unimplemented.

Verify actual selection, responsiveness and file output under Proton and native Windows, and confirm PR merge separately. Reconsider host-specific selection handling if compatibility or registry restoration fails, queue sizing if memory or loss is unacceptable, and colors if readability suffers. Full pre-compaction discussion, original failure cause, final runtime binary identity and native Windows results remain unavailable.
