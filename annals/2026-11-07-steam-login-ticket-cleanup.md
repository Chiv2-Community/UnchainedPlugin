# Scope Steam ticket cleanup to the Redpoint login probe

- Date: 2026-11-07
- Status: implemented on a PR branch; not established as merged
- Scope: Steam authentication, ticket lifecycle, executable compatibility
- Repository: [UnchainedPlugin](https://github.com/Chiv2-Community/UnchainedPlugin)
- Recording HEAD: `0c1e9f14e8d7d63657846016ec5295a225ef7e2e`, branch `fix/steam-auth-ticket-leak`; clean working tree before this record.
- Exact tag: none; nearest tag: `v1.0.0-RC3`. Base: `4905eaa3aef5ef46c31f6fded116d1999204ec24`. Implementation: `0c1e9f14e8d7d63657846016ec5295a225ef7e2e`; [PR #27](https://github.com/Chiv2-Community/UnchainedPlugin/pull/27), open at source response.
- Sources: user and build agent, session `ses_e5658faf6ffewA56PWUxX9QcPT`, through response `msg_1b75be7bb0012I5eU9L6ydgSq5`; earlier conversation was compacted and is not fully available. Local diagnostic artifacts are ephemeral, not repository history.

## Decision and context

The user requested a plugin workaround for a base-game ticket leak, and asked that it use the project's standard hook macros. The implementation scopes cleanup to Redpoint's login predicate: request a temporary session ticket, preserve the predicate's success/failure check, and cancel the handle immediately. Other token callers retain their original path. This is a workaround in the plugin, not a modification to the shipping executable.

The investigation initially left Steam's rejection reason unknown. A later live Steam trace identified a per-app Web API ticket quota: 24 active entries against a limit of 20, returning invalid handle 0. The game separately registers that invalid handle without completing the callback, leaving EOS login waiting. A traced Redpoint `GetAuthToken()` call allocated a session ticket whose handle was discarded; a diagnostic launch increased the count from 24 to 25 and its ticket remained after forced termination. This demonstrates an accumulation path, not the origin of every pre-existing ticket. The trace and inventory are recorded in `/home/jbarber/.cache/opencode/chiv2-frontend-hang-20261107/steam-side-root-cause.md` and `steam-ticket-quota-trace.txt`.

## Options and rationale

- Reuse an existing ticket: the user asked about this. The agent found no safe cache because the game discards handles; a plugin-owned cache would need expiry, logout and cancellation management. A session ticket cannot replace the distinct EOS Web API ticket.
- Cancel generic `GetAuthToken()` tickets immediately: rejected because other callers may need them for authentication.
- Target only the length-check predicate: chosen to retain its observed behavior without changing other callers' ticket lifetimes. A thread-local identity/user scope limits interception. The user preferred standard `CREATE_HOOK!` and `CALL_ORIGINAL!` over custom registration.
- Independently handle invalid Web API handles and a finite timeout: discussed as additional defenses, not implemented by this PR.

The build is restricted using platform, CRC32C and code signatures for the diagnosed executable. Compatibility gating avoids requiring the new signatures on unknown and Epic builds. The implementation and assumptions are in [the design documentation](../docs/steam-auth-ticket-cleanup.md) and [hook code](../src/resolvers/steam_auth/mod.rs).

## Evidence, consequences, and follow-up

The source transcript reports 11 passing standalone policy and executable-signature tests and a Windows x64 release build, with a temporary local resource-tool override excluded from the commit. The recorded live test log shows three isolated launches with counts `24 -> 24 -> 24` before, after the probe and after forced exit; the unpatched control had retained one new ticket. These tests do not establish full EOS login recovery. Installed plugin and original server were reported unchanged; the hook was tested via isolated injection.

The workaround does not reclaim existing tickets, repair an already hung login, or fix invalid Web API callbacks. Restarting Steam was not tested as a controlled recovery in this investigation. Validate full authentication, failure and timeout behavior, and unknown-build startup separately; re-evaluate signatures and ABI assumptions when the game changes. Security setting restoration after debugging was not confirmed in the supplied history.

## Open questions

Whether all historical tickets arose from this path and whether a Steam restart reliably restores login remain unverified. The compacted source does not preserve all earlier investigative steps. PR merge and deployment after the source response are outside this record.
