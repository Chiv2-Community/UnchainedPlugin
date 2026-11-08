# Steam callsite patch and resolver platform coverage

- Date: 2026-11-07
- Status: implemented on PR branch; merge not verified
- Scope: Steam authentication, platform-specific signature resolution, Windows plugin tests
- Repository: https://github.com/Chiv2-Community/UnchainedPlugin.git
- Recording HEAD: `c83d99ef212660b7edf8bbb745054560afacbf4e`; branch `fix/steam-auth-ticket-leak`; working tree clean before recording. No exact tag; nearest tag `v1.0.0-RC3`.
- Implementation: `0a83920bc96c86838b3c387a0e1d3f5ff5bcdb71` (callsite); `08e786b088a5a8732bae29127e34e3f6056c66f8` (platform inference); `f345901284926e5f4f811dd039cb010296affa2d` (Windows tests); `c83d99ef212660b7edf8bbb745054560afacbf4e` (declarative macro). [PR #27](https://github.com/Chiv2-Community/UnchainedPlugin/pull/27).
- Sources: supplied conversation checkpoint and committed code and documentation; earlier source session `ses_e5658faf6ffewA56PWUxX9QcPT`, previously recorded through response `msg_1b783fe450018v1lo7fXOWL4ft`. The later checkpoint's session and response IDs and complete conversation are unavailable. Local diagnostic artifacts are unversioned.

## Decision and context

Replace the Steam-only allocate-and-cancel hook with a signature-located patch that skips Redpoint's unused token allocation and length check, retaining the preceding Steam login-status check. The earlier hook and its bounded lifecycle evidence are described in [Steam ticket lifecycle and concurrent instances](2026-11-07-steam-ticket-lifecycle-evidence.md). The alternative retained the length check but still allocated a ticket; the new patch removes the requirement that this unused allocation succeed. The committed [explanation](../docs/steam-auth-ticket-cleanup.md) attributes the choice to the token authenticating nothing in this check and EOS obtaining credentials separately. This is a plugin workaround, not a general ticket cleanup or a fix for invalid Web API callback handling.

Resolver applicability is inferred from platform signature declarations; missing Steam or EGS coverage produces a warning, and intentional omissions can be annotated without enabling them. Shared patterns remain shared. Windows plugin tests use the production `PlatformType`, avoiding a divergent test enum. The final implementation places registration and warnings in `define_pattern_resolver!`, replacing a separate procedural macro. The checkpoint identifies simplification as the reason; further contemporaneous rationale and alternative discussions are unavailable. See [macro](../src/macros/sig_macros.rs) and [platform tests](../src/resolvers/platforms.rs).

## Evidence and consequences

The [patch](../src/resolvers/steam_auth/mod.rs) is Steam-only and jumps over the token check; the signature and machine-code assumptions require reconsideration when the executable changes. The historical documentation at `0d656fffca0b9f02857b5971e47d7e4794e36936` records a Proton callsite test, including observed patch bytes and local game travel, but its debugger failed before login tracing. Later documentation at `ee89c4353328ed9074cc079ae5839931af3c84e7` reports additional Steam multiplayer testing. The checkpoint reports six traced client logins and five host runs with unchanged active-handle baseline; those counts were not independently verified here. Snapshots cannot exclude transient allocations. Earlier allocate-and-cancel tests do not validate this patch. The user requested concise current documentation without run-specific validation history; `a803a4aa2bd7cdb6197c8d7a3809b1dab2fb97b0` condensed it.

Frontend post-load and `InProgress` alone did not establish a playable server; historical documentation required the intended map and listening port. A console-exit access violation is tracked in [issue #29](https://github.com/Chiv2-Community/UnchainedPlugin/issues/29); its cause and relationship to the patch are unknown. Repository-wide signature testing is tracked in [issue #28](https://github.com/Chiv2-Community/UnchainedPlugin/issues/28); remote issue and PR states were not checked. Native Windows and official matchmaking remained untested in the historical documentation. Reconsider on signature, Steam, or authentication behavior changes, and validate supported builds and clean shutdown separately.

## Open questions

The complete source discussion and later response identifiers are unavailable. This review did not rerun tests or inspect all external logs; reported run counts and current deployment remain unverified. The patch cannot remove already retained tickets or guarantee recovery from an existing hang.
