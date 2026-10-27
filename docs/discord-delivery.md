# Discord delivery

All outbound Discord traffic uses `features::discord::outbound::DiscordOutbox`.
Subscribers format or enqueue publications; they never await Discord HTTP.

- Each configured channel has one batching worker sharing Serenity's HTTP client and rate limiter.
- Interactive messages have 64 reserved queue slots. Event logs have 256 slots.
  Full or closed queues discard new publications and report aggregate drops.
- Workers favor interactive messages, but service logs between message bursts.
  HTTP requests run in background tasks; collection and timers never await them.
  Each channel allows at most 64 outstanding requests, including limiter waits.
  At capacity, new messages/batches are dropped with a warning; dashboards stay pending.
  Discord's shared rate limiter can still delay requests.
- Event logs are collected in FIFO order, batching up to 10 embeds or 6000 UTF-16 units.
  Full 10-event batches flush immediately when the worker is free. Partial batches
  flush after one second without a new event. Each arrival resets this inactivity
  timer; empty batches have no active timer.
- Dashboard updates coalesce to the latest pending state. Each channel maintains
  one dashboard message, with updates spaced at least five seconds apart.
- Only one dashboard request is outstanding at a time to retain its message ID
  and avoid overlapping creates/edits. Other messages do not wait for it.
- Concurrent requests can complete out of order, including event-log batches.
  Ordering is not guaranteed between interactive messages, dashboards, and logs.
  Command output can precede its audit records. Queueing is not durable storage;
  failed requests are logged and discarded, and process exit can lose pending work.

HTTP requests have a 10-second connect timeout and a 30-second total timeout.
These do not cancel Serenity's required rate-limit waits. A request pending for
10 seconds emits a warning; rate-limit callbacks report the requested wait.
Debug logs report queue age and delivery duration without message contents or tokens.
Diagnostics use the shared `sdebug!` and `swarn!` macros, including function metadata
for custom `{fn}` patterns. The default DEBUG level includes these timings;
TRACE remains disabled. See [logging configuration](logging.md).

## Diagnostic stages

- Worker/channel IDs, queue depths, batch size and UTF-16 units, oldest-event age,
  and flush reason (`inactivity`, `embed_limit`, `character_limit`, or `shutdown`).
- Delivery IDs correlate send start, HTTP response-header arrival, pending warnings,
  completion, and transport-error flags. HTTP headers arriving promptly followed
  by a long completion time implicates response-body handling, not batch collection.
- `X-RateLimit-Limit`, `X-RateLimit-Remaining`, and `X-RateLimit-Reset-After`
  are logged as **Serenity parsed/cached bucket snapshots**, not raw response dumps.
  Snapshots may reflect earlier responses or pre-request decrements. `None` means
  unknown, not an unlimited quota. A busy bucket is reported without waiting on its lock.
- An actual HTTP 429 emits `Discord HTTP 429: rate limited` before Serenity retries
  it. A limiter callback can also mean a pre-emptive wait without any 429; these
  are logged separately. Transport timeouts are not labelled as rate limiting.

Reqwest's response classifier observes statuses without retrying request bodies;
Serenity alone retains responsibility for 429 retries and backoff. No batching,
queue capacity, or scheduling policy changes are needed to enable these diagnostics.

Discord's 50 requests/second global limit does not override per-route/channel
limits. Use the reported bucket values and wait duration, not a fixed send rate.

A multi-minute gap is not the batch timer. Check rate-limit wait warnings,
pending-request warnings, request failures, and queue-drop reports before changing
batch sizes. Increasing queue capacity only permits a larger stale backlog.
