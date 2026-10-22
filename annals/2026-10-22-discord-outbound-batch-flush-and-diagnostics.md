# Discord outbound batch flush policy and delivery diagnostics

- Date: 2026-10-22
- Status: implemented (uncommitted)
- Scope: Discord event-log batching and outbound delivery observability
- Repository: UnchainedPlugin
- Canonical remote: https://github.com/Chiv2-Community/UnchainedPlugin.git
- Branch: `fix/discord-outbound-bus`
- Recorded at commit: `292d90439921a730665e194b1b1ac7e070442c41`
- Nearest tag: `v1.0.0-RC3-12-g292d904`
- Implementation provenance: uncommitted working tree; no implementation commit, tag, or deployment
- Source: session `ses_ed7be2d91ffeonK4Di8UKbHXLI`, response `msg_16497d98c001X3K5PxWhtbbghk`; build agent response and its recorded tool evidence

## Decision summary

Use ten embeds as the immediate event-log batch threshold. Flush a smaller pending batch on a five-second worker timer that is reset after every event-batch send attempt, not when events arrive. Do not intentionally collect fifty events or five Discord messages before dispatch.

Add delivery diagnostics that distinguish queued work, Serenity's parsed rate-limit state, observed HTTP 429 responses, and transport failures without logging tokens, message contents, URLs, or raw headers.

## Context

The user observed apparent bursts of five Discord messages containing ten events each and suspected a fifty-event buffer. The existing outbound worker and batch type were inspected. `Batch` caps messages at ten embeds (and 6000 UTF-16 units), and the worker flushed a full batch at ten; no fifty-event threshold was found.

The existing partial-batch behavior instead used a one-second deadline started by the first event. The requested operational policy was a five-second cadence reset by each batch flush. The user also requested rate-limit header information and explicit 429 logging while investigating delivery stalls.

## Evidence and constraints

- Source inspection showed `MAX_EMBEDS = 10` and a full-batch send path; it did not show a fifty-event threshold.
- A live, earlier version of the outbox connected to Discord but had repeated approximately 30-second HTTP failures and queue ages up to 73 seconds while the game and A2S remained responsive. This established that a channel worker blocked on its current request can accumulate a backlog, but did not prove why five messages were observed.
- Discord's global 50-requests-per-second figure does not replace per-route or per-channel limits. Rate-limit waits and transport timeouts must therefore be observed separately.
- Serenity owns Discord rate-limit backoff and retries. The added reqwest response classifier observes response status without retrying bodies so it does not create a competing retry policy.
- The conversation history before the supplied compacted segment is unavailable. No claim is made about earlier rationale beyond the evidence above.

## Options considered

### Add a fifty-event threshold

Rejected. Inspection found no such threshold, and adding one would deliberately increase latency without explaining the observed bursts.

### Keep the one-second timer started by the first event

Rejected by the user-requested policy. It did not provide the requested five-second post-flush cadence.

### Reset the timer on every event arrival

Rejected. Under continuous traffic this can indefinitely defer a partial batch.

### Cancel and retry pending HTTP work outside Serenity

Rejected. It could interfere with Discord-required backoff and cannot establish that a timeout was rate limiting.

## Chosen implementation and rationale

The uncommitted implementation changes `FLUSH_INTERVAL` to five seconds. A full ten-embed batch is sent immediately when the worker is free. Deadline, character-limit, and embed-limit send attempts reset the next partial-batch deadline; event arrivals do not. Empty timer ticks send nothing.

It adds worker and delivery IDs; queue depths and ages; batch size, UTF-16 count, and flush reason; parsed/cached `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and `X-RateLimit-Reset-After` snapshots; and a distinct observed-429 warning. Cached bucket snapshots are explicitly not raw response headers and may be stale. Transport failures remain labeled as transport failures, not as rate limiting.

Confidence: high for the absence of a fifty-event threshold and the implemented batch timing, based on the inspected source and regression tests. Low for any causal explanation of the five-message pattern or the original multi-minute stall; those remain unproven.

## Consequences and follow-up

- Event logs retain FIFO order, ten-embed/6000-unit limits, bounded queues, and best-effort delivery.
- A single stalled request still blocks that channel worker, so queued events can later drain as full ten-event batches. Other channel workers and game-event publication are not blocked by that request.
- The modified diagnostics and timing policy were not deployed to the running server described in the source session. The earlier live DLL does not validate these subsequent changes.
- Run the updated DLL under sustained traffic and use the new queue, bucket, 429, and transport diagnostics before changing batch capacity or attribution of the stall.
- Reconsider the scheduling policy if observed diagnostics show worker fairness or route limits, rather than stalled requests, are the cause of unacceptable latency.

## Implementation and validation status

Uncommitted changes include `src/features/discord/outbound.rs`, `src/features/discord/diagnostics.rs`, `src/features/discord.rs`, `src/modules/discord/event_log/batch.rs`, `docs/discord-delivery.md`, and supporting integration changes. The repository working tree also contains other uncommitted Discord-outbox changes; this annal does not assert they are all attributable to this decision.

The recorded Windows release cross-build completed successfully. The isolated Wine test run reported 90 passed, 3 pre-existing ignored, and 0 failed, including the immediate ten-event flush, post-flush timer reset, observed-429, and rate-limit snapshot coverage. `git diff --check` also passed. Native Linux library testing remains blocked by the existing `retour` unsupported `stdcall`/`fastcall` ABI issue and was not evidence for this change.

## Open questions

- What caused the observed groups of five messages remains unknown.
- The original multi-minute delivery pauses and live game-process HTTP failures remain undiagnosed.
- No post-change live-server validation or implementation commit exists at recording time.
