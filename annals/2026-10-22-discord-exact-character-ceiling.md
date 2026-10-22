# Flush Discord batches at the exact character ceiling

- Date: 2026-10-22
- Status: implemented
- Scope: Discord event-log batching
- Repository: UnchainedPlugin
- Canonical remote: https://github.com/Chiv2-Community/UnchainedPlugin.git
- Branch: `fix/discord-outbound-bus`
- Implementation commit: `0b6a0289e1db432ff26bac9ef32e44fbf2a7f636`
- Nearest tag: `v1.0.0-RC3-19-g0b6a028`; no tag points at the commit
- Related decision: [Discord inactivity batching and live validation](2026-10-22-discord-inactivity-and-live-validation.md)
- Evidence: implementation commit and its regression test; no source conversation, issue, or review comment was available in the supplied context

## Decision summary

Flush an event-log batch immediately when it reaches either the ten-embed limit or the 6000 UTF-16-unit limit. The character ceiling is inclusive: a batch exactly at 6000 units must not wait for the inactivity timer.

## Context and evidence

The prior `Batch::fits` check rejected an entry that would take a batch above 6000 units, but `Batch::is_full` only considered the ten-embed limit. Consequently, an entry that made a batch exactly 6000 units remained pending until another event, shutdown, or the one-second inactivity deadline.

Commit `0b6a028` adds the character condition to `is_full`, exposes the limit to the worker for accurate flush diagnostics, and adds a paused-time regression using formatted chat embeds of exactly 2000 units each. The test verifies three such embeds flush at 6000 units without elapsed inactivity time.

## Options considered

No discussion of alternatives was retained. The implementation evidence supports correcting the threshold behavior rather than changing the 6000-unit ceiling or the one-second inactivity policy.

## Consequences and follow-up

- Exact-limit batches have lower latency and retain the existing 6000-unit maximum.
- Full-batch diagnostics now distinguish `character_limit` from `embed_limit`.
- The 10-embed, 6000-unit, and one-second inactivity policies otherwise remain unchanged.
- No validation result beyond the committed regression test was available. Native Linux library tests remain subject to the existing `retour` calling-convention blocker.

## Reconsider when

Reconsider only if Discord's actual embed accounting differs from the UTF-16 accounting used by `Batch`, or if production diagnostics show a different batching constraint.

## Open historical gaps

The available checkpoint did not retain the review finding, user rationale, test-run output, source session ID, or response ID that led to this correction.
