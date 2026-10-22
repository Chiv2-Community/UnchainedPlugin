# Discord inactivity batching and live validation

- Date: 2026-10-22
- Implementation: `1683ccf` on `fix/discord-outbound-bus`, based on `292d904`.
- Supersedes: [five-second flush policy](2026-10-22-discord-outbound-batch-flush-and-diagnostics.md).

## Final policy

- Dispatch immediately at ten events or before exceeding 6000 UTF-16 units.
- Dispatch partial batches after one second without another event. Each arrival resets the timer.
- Dispatch HTTP through bounded background Tokio tasks, not new OS threads. Collection never awaits responses.
- Keep dashboard requests single-flight and coalesce updates. Concurrent message completion can reorder batches.
- Keep Serenity's backoff. Log request outcomes, observed 429s, queue/batch timings, and parsed/cached rate-limit snapshots.

The user replaced the earlier five-second policy with this inactivity rule and explicitly requested detached HTTP delivery. Inspection found no fifty-event buffer.

## Runtime finding

The launcher emits `--discord-mention-admins false`. The plugin previously treated that option as a valueless boolean: `false` started trailing positional arguments and swallowed later options, including `--desync-patch`. Explicit boolean parsing now accepts true/false while retaining bare-flag compatibility. A deterministic regression covers both values and subsequent desync/headless flags.

## Validation

- Windows release DLL built. Isolated Wine suite: 92 passed, 3 existing ignored, 0 failed.
- Verified 98 exported Rust/manifest/lock files against the worktree, excluding the build helper's resource-toolkit adjustment.
- Verified mapped DLL SHA-256: `0731579ed40eb8d9afc3b1782c62a4de0391bf5ac170b3b7f5d3aedcf2c1082a`.
- Local launcher CLI `8be3d51` hosted `FFA_Courtyard` headless with the desync patch confirmed applied and 20 bots.
- Three-minute observation: 13/13 A2S queries succeeded; 50 inactivity flushes and 3 full ten-event flushes; batch sizes covered 1–10.
- Discord REST confirmed 56 messages in the latest 100 after launch. No observed 429s, queue drops, or dispatch drops. Maximum outstanding requests: 8.
- Recorded 17 request failures; successful delivery and nonblocking batching do not prove the underlying HTTP stalls resolved.
- User reported delivery looked good and requested teardown. Test server stopped; logs preserved outside the repository.

Evidence: `/home/jbarber/.cache/opencode/discord-outbound-headless-v2-20261022/` (`summary.json`, `samples.json`, `source-hashes.json`, `mapped-dll.json`, `discord-messages.json`, and redacted logs). Private configuration and credentials are excluded from the repository.

Native Linux library tests remain blocked by `retour`'s unsupported `stdcall`/`fastcall` ABIs. Live results cover this pre-review implementation, not later review fixes.
