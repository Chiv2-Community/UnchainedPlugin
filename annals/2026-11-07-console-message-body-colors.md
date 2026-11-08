# Console debug and trace message-body colors

- Date: 2026-11-07 local (2026-11-08 UTC).
- Status: accepted; implementation uncommitted.
- Scope: default console message-body colors.
- Repository: [UnchainedPlugin](https://github.com/Chiv2-Community/UnchainedPlugin.git); branch `log-standardization-finalization`; recording HEAD `84b1e7396a318a0152e1cf7dd7186a7fd2a74797`; no exact tag; nearest tag `v1.0.0-RC3`. Committed baseline `2c1649a027eadf0962c80567b25fe79bc297a601`, [PR #30](https://github.com/Chiv2-Community/UnchainedPlugin/pull/30).
- Source: supplied checkpoint of user and build-agent conversation `ses_e5658faf6ffewA56PWUxX9QcPT` through response `msg_1b9a1266d001RzQ9JFR4w1vA34`; earlier history is partly compacted. The rationale and exact wording are unavailable.

## Decision and context

The checkpoint reports that the user requested debug and trace message bodies match their respective gray level colors. This supersedes neutral message bodies for those two levels in the [function and separator color decision](2026-11-07-console-function-separator-colors.md) and the earlier [field-color decision](2026-11-07-console-fields-wine-selection.md). It resolves the uncertainty in [the pipe decision](2026-11-07-console-level-pipes.md), which had observed the code change without establishing a user preference. Neutral bodies remain for error, warning, and info. The user's reason and any other considered alternatives are not available; the prior neutral-body option is the documented previous choice, not evidence of a fresh comparison.

The same checkpoint confirms the previously recorded gold `#BE9D5C` approval and trace-colored pipes. See [gold revision](2026-11-07-console-gold-revision.md) and [pipe decision](2026-11-07-console-level-pipes.md); these choices are not new here.

## Evidence, consequences, and follow-up

The uncommitted `src/logging/log4rs/log4rs.default.yaml` and `palette.rs` select `{l}_body`: debug is `#808080`, trace `#5c5c5c`, and other default bodies `#eeeeee`. The updated palette test asserts the corresponding output; `docs/logging.md` describes the layout. These three files had pre-existing uncommitted changes at recording; the committed baseline does not include this revision.

The external `pipes-debug-trace-tests.log` reports 41 passed and 7 ignored logging tests, including the palette defaults test. The checkpoint attributes this run to Proton; the log does not independently pin the tested binary or working-tree content. The earlier Steam preview initialized with the gold shade, but the later pipe and body changes required a relaunch; visual validation of those changes is not established. Native Windows selection remains untested, and the prior Wine synthetic drag establishes highlighting, not native behavior.

Review and test the complete diff, relaunch to validate rendering, and commit through the normal PR process if appropriate. Reconsider if gray body text is insufficiently readable or console compatibility fails. The full conversation, precise rationale, and final runtime artifact remain unavailable.
