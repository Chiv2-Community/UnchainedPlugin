# Trace-gray pipes around console level

- Date: 2026-11-07 local (2026-11-08 UTC).
- Status: accepted; implementation uncommitted.
- Scope: console log field separators and observed message-body formatting.
- Repository: [UnchainedPlugin](https://github.com/Chiv2-Community/UnchainedPlugin.git); branch `log-standardization-finalization`; recording HEAD `5685a30ea520564e4872ce79d92940a8c6614bd9`; no exact tag; nearest tag `v1.0.0-RC3`. Earlier format implementation: `2c1649a027eadf0962c80567b25fe79bc297a601`, [PR #30](https://github.com/Chiv2-Community/UnchainedPlugin/pull/30).
- Source: supplied compacted conversation checkpoint, user and build assistant; source session and response IDs for this pipe request were not supplied. Earlier conversation is partly unavailable. Implementation and documentation changes in the three files below were uncommitted at recording.

## Decision and context

After approving the gold preview documented in [Console context gold revision](2026-11-07-console-gold-revision.md), the user requested trace-gray pipes before and after the level. The intended layout is `timestamp | LEVEL | short_module::function - message`. This changes the single timestamp-gray pipe after the level in the committed [format implementation](2026-11-07-console-selection-format-implementation.md) and the separator presentation in [Console function and separator colors](2026-11-07-console-function-separator-colors.md). The ` - ` separator remains timestamp gray. The user's further rationale is unavailable; the earlier single pipe was the baseline, not a documented separately evaluated alternative.

## Evidence and consequences

The uncommitted `src/logging/log4rs/log4rs.default.yaml` and built-in pattern in `src/logging/log4rs/palette.rs` both set the two pipes to the trace palette (`#5c5c5c`); `docs/logging.md` describes them, and palette test assertions are updated. The same diff selects `{l}_body` for messages, making debug and trace bodies gray while other bodies remain neutral; this differs from the neutral message text in [the previous color decision](2026-11-07-console-function-separator-colors.md). The supplied checkpoint does not establish a user decision or rationale for this body-color change, so it is an observed uncommitted implementation, not a confirmed preference.

The checkpoint reports the final pipe change had not been rebuilt or relaunched. The earlier 175-passed, 16-ignored Proton suite predates the final saturation and pipe edits and does not validate them. This review did not run tests. Rebuild, run the relevant palette tests and visually verify the complete layout before committing through the PR; native Windows selection remains untested. Reconsider the pipes if readability or host rendering is inadequate, and clarify whether the body-color change is intended. Source IDs and full discussion remain unavailable; the preview approval does not prove the later pipes were visually reviewed.
