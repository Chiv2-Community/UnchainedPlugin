# Unpadded combined console context preview

- Date: 2026-11-07 local (2026-11-08 UTC).
- Status: proposed; preview only.
- Scope: default console log presentation.
- Repository: [UnchainedPlugin](https://github.com/Chiv2-Community/UnchainedPlugin.git).
- Recording branch: `log-standardization-finalization`; HEAD `4d75ea2e766166e8d582eec8ad285423bdc48de7`; no exact tag; nearest tag `v1.0.0-RC3`. Implementation baseline `3cfc8c1db58323970a39207401949434c06c50f6`, [PR #30](https://github.com/Chiv2-Community/UnchainedPlugin/pull/30). Working tree clean before recording.
- Source: user and build agent, session `ses_e5658faf6ffewA56PWUxX9QcPT` through response `msg_1b972c69a0015tFlZy4vp1N6vJ`; earlier history is compacted.

## Context and choice

After the [logging standardization](2026-11-07-console-logging-standardization.md), the assistant suggested milliseconds to distinguish bursts and diagnose pauses. The user requested a preview combining timestamp, level, abbreviated module, and function in one severity-colored header. The assistant corrected a missing palette brace for the experiment. The user then asked to remove the module's fixed width; the preview uses `{sm}::{fn}` without padding. The existing short-module implementation still caps the abbreviation at 32 Unicode characters.

The experimental console pattern is:

```text
{palette({l})}[{d(%H:%M:%S%.3f)} {l:>5} | {sm}::{fn}]{palette({l}_body)} {m}{n}
```

The preceding preview used `{sm:<-24}::{fn}`, which padded before the function separator. Removing padding avoids that gap but gives up fixed column alignment. The committed PR pattern instead has separate module and function brackets and a fixed 24-character module field. The request was handled as a preview, not acceptance of a new repository default; it does not supersede the earlier record's committed-format decision.

## Evidence and consequences

The supplied transcript shows the change made in an external validation source copy, followed by a successful Windows release build and a Steam preview launch using DLL SHA-256 `7bbc4d9c0159018f6704d41f6f7136e14d81f3365329828807a1ee59a28cb0e8`. The launch log check reported plugin initialization. This does not independently establish visual rendering, selection responsiveness, or user acceptance. Earlier EGS and Steam preview attempts stalled; a subsequent Steam retry initialized. The assistant reported the installed DLL unchanged, but the final launch excerpt does not independently verify its hash.

The repository baseline still contains the fixed-width default in `src/logging/log4rs/log4rs.default.yaml` and `src/logging/log4rs/palette.rs`; the experiment has no implementation commit on this branch. Native Windows and final live selection remain unverified. Reconsider the default only after user review of the preview and validation of formatting, tests, documentation, and compatibility. No additional alternatives or rationale are recoverable from the compacted history.
