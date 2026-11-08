# Steam ticket lifecycle and concurrent instances

- Date: 2026-11-07
- Status: superseded (the cancellation-hook implementation); experimental findings are bounded to the tests below
- Scope: Steam authentication probe in UnchainedPlugin
- Repository: https://github.com/Chiv2-Community/UnchainedPlugin.git
- Recording branch: `fix/steam-auth-ticket-leak`; HEAD: `f345901284926e5f4f811dd039cb010296affa2d`; no exact tag; nearest tag: `v1.0.0-RC3`.
- Source implementation: `5bf490c55bd0b75264bd7f08babe4be06b1a9b46`; later replaced by `0a83920bc96c86838b3c387a0e1d3f5ff5bcdb71`.
- Source: user and build agent, session `ses_e5658faf6ffewA56PWUxX9QcPT`, through response `msg_1b783fe450018v1lo7fXOWL4ft`. Earlier conversation was compacted; its full history is unavailable. Diagnostic artifacts are local, unversioned files under `chiv2-frontend-hang-20261107`.
- Working tree at recording: unrelated resolver and macro edits were uncommitted; they were not used as evidence or changed.

## Decision and context

At the source commit, the Steam-only workaround captured the Redpoint login predicate's temporary session-ticket handle and cancelled it promptly, preserving the predicate's token-length check. Other authentication calls were left alone. The aim was to prevent unnecessary outstanding tickets from contributing to the Steam per-app quota, without cancelling tickets needed by other consumers. The source summary reports that ticket reuse was rejected because handles were discarded and session tickets cannot substitute for EOS Web API tickets; the full discussion is unavailable.

At recording HEAD, the committed implementation instead skips that token check and its allocation altogether; `0a83920` supersedes the earlier hook. This later implementation is verified in Git, but its decision conversation is outside the supplied response and its rationale is not reconstructed here. See [PR #27](https://github.com/Chiv2-Community/UnchainedPlugin/pull/27) and [signature-testing issue #28](https://github.com/Chiv2-Community/UnchainedPlugin/issues/28); their current remote status was not checked.

## Evidence and corrections

The original investigation attributed a Web API ticket rejection to 24 outstanding session tickets against a 20-ticket Web API threshold and traced a Redpoint token check to an allocation whose handle the game discarded. The origin of the 24 pre-existing tickets was not established. The old claim that immediate post-SIGTERM snapshots proved lasting retention was withdrawn: in single-game controls with a non-Steam Wine-session holder, normal `RequestExit(false)`, SIGTERM, and Windows `TerminateProcess` each returned the inventory from 24 to 25 to 24, with cleanup observed about 0.3 seconds after exit. A separate trial showed cleanup can be delayed. These observations do not establish cleanup on every exit path.

The user asked whether cancelling twice or shutting down after cancellation would break anything, and whether concurrent game instances affect cleanup. Three live Steam API v157 trials repeatedly cancelled a diagnostic handle, preserved other active handles in the inventory, and completed `SteamAPI_Shutdown` with the exact baseline restored. This is observed behavior for that build, not a documented idempotency guarantee or proof that every ticket remains usable for authentication.

The first two-unpatched-instance test found the first instance's diagnostic ticket still present after both processes exited, including a 10-second wait; the other instance's ticket disappeared. This refutes a general inference that every process exit cleans its tickets, but does not establish the mechanism or explain the old baseline. Longer and patched-instance comparisons were still running at the source response; do not treat that matrix as passed. Evidence: local `ticket-lifecycle-concurrent.log` and `graceful-result-{5-session-held,6-forced-session-held,7-session-held,8-windows-forced}.json`.

## Consequences and follow-up

At the source commit, nine policy tests passed and a simulated 32-thread, 32,000-probe stress test passed; these are not end-to-end authentication tests. The existing 24-ticket quota prevented validating successful EOS login or multiplayer authentication. Reconsider the workaround and compatibility assumptions when game signatures, Steam behavior, or concurrent-instance evidence changes; validate successful authentication and ticket isolation on supported builds. Whether duplicate cancellation is contractually safe across Steam versions and what causes concurrent retention remain open. The source agent reported that restoring `kernel.yama.ptrace_scope=1` required user authorization; current host state was not checked.
