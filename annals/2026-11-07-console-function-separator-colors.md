# Console function and separator colors

- Date: 2026-11-07 local (2026-11-08 UTC).
- Status: accepted; implementation uncommitted.
- Scope: default console presentation and Wine selection limitations.
- Repository: [UnchainedPlugin](https://github.com/Chiv2-Community/UnchainedPlugin.git); branch `log-standardization-finalization`; recording HEAD `659a33bf141409270597f5fd46819da906a3181b`; no exact tag; nearest tag `v1.0.0-RC3`. Baseline implementation `3cfc8c1db58323970a39207401949434c06c50f6`, [PR #30](https://github.com/Chiv2-Community/UnchainedPlugin/pull/30). Seven implementation and documentation files were modified and uncommitted at recording.
- Source: user and build agent, session `ses_e5658faf6ffewA56PWUxX9QcPT` through response `msg_1b987d186001MbEke35cAKXvD7`, supplied as a checkpoint summary. Earlier conversation is partly compacted; full discussion is unavailable.

## Decision and change

The user chose a bracket-free layout with gray millisecond timestamps and separators, severity-colored level, gold `#B49A66` module, `::`, and function, and neutral message text. This changes the earlier [field-color decision](2026-11-07-console-fields-wine-selection.md), which used neutral function text; the separator is now explicitly gray. The working-tree built-in pattern, shipped YAML, palette and logging documentation reflect the revision. Its rationale beyond the user's choice is unavailable; no additional alternatives are established.

Wine selection remains rectangular. The supplied checkpoint reports that line-based selection would require a different host or custom support; the available evidence does not establish a chosen implementation for either alternative. The queue and Wine QuickEdit choices in the earlier records are otherwise unchanged.

## Evidence and consequences

The uncommitted `src/logging/log4rs/palette.rs` pattern colors `{sm}::{fn}` as a unit and switches to timestamp gray for ` - `. `docs/logging.md` describes the same format. The external `function-separator-colors-tests.log` reports the focused shipped-and-built-in palette test passed. The earlier 175-passed, 16-ignored Proton suite preceded this color-only revision, not a full test of the final working tree. The checkpoint reports the running preview still has the previous colors and needs rebuilding and relaunching; no current visual confirmation or binary hash is established.

The change remains outside the committed baseline. Review and test the complete diff, rebuild the preview, and verify user-visible rendering and selection before committing through the normal PR process. Native Windows behavior and human selection remain unverified. Reconsider colors if readability or host compatibility fails, and selection strategy if line-based selection becomes a requirement. Full historical rationale and a final runtime artifact remain unavailable.
