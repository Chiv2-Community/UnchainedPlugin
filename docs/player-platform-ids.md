# Validated player identities

The public-data challenge replaces the earlier credential-forwarding prototype. See the
[credential-free validation investigation](playfab-credential-free-validation.md)
for upstream PlayFab schemas, public SDK references and alternative designs.

Opt-in fix for missing PlayFab IDs on user-hosted servers. The normal browser
path proves account control through PlayFab user data and obtains a backend-verified,
single-use ticket before native travel. The
server redeems it using its live registration, then runs the original native
completion and typed PlayFab setter. Claimed URL/platform IDs are not proof.

## Configure

Use a backend with `GAME_JOIN_TICKET_DATABASE`, `GAME_JOIN_AUDIENCE` (the canonical
backend URL without trailing slash) and `GAME_JOIN_VERIFIER_SESSION_TICKET`
configured. The last setting is a separate verifier account's legitimate PlayFab
session, provisioned by the operator, never collected from joining players.
On both client and server, set:

```text
--server-browser-backend https://your-backend.example
--validated-player-identity
```

The server also needs `--register` and its normal hosting arguments. Join through
the in-game browser. The second option authorizes temporary public writes to the
reserved PlayFab user-data key `UnchainedJoin_v1`, using the configured browser backend.
HTTPS is required except for literal loopback tests.
Credentials in URLs, query strings and fragments
are rejected. Ordinary browser configuration alone never forwards credentials.

The player's session goes only to `https://EBF8D.playfabapi.com`, never to the
browser backend or game server. The client publishes a domain-separated hash
bound to its account, backend, server, fresh challenge and private-secret
commitment. The backend reads it through PlayFab using its independent verifier
session. Completion requires the private client secret, not just the publicly
readable hash. Game servers receive only a short-lived, server-bound grant.

Only the reserved key is changed, and cleanup is attempted and checked even if
issuance fails. Unrelated pre-existing data under that key blocks the join.
Prior protocol commitments can be replaced; crashes/network failures can leave
a public hash. Concurrent joins/backends can fail closed because PlayFab has no
compare-and-swap here. No other user-data keys or permissions are changed.

The operator must legitimately provision and renew the verifier session; no
automatic login, account creation or extraction mechanism is shipped. An expired
verifier login makes admission unavailable. This custom account-control proof
does not establish every storefront entitlement or Torn Banner admission policy.

Validation is fail-closed: no backend/credential/ticket means no validated join.
Servers opting in reject missing, legacy and malformed tickets in PreLogin;
expired, replayed and mismatched grants fail native confirmation. Retry through
the browser for a fresh ticket; consumed tickets are never retried automatically.
Without opt-in, existing native behavior is unchanged.

## Internals

- Uses backend-owned generated challenge/issue/redeem operations; no copied
  browser-backend contract or wire DTOs. Direct PlayFab calls follow its pinned
  public schemas/SDKs linked in the investigation document.
- `ucj1.` is our chosen version marker for Unchained join grants, not a value
  recovered from the executable. It separates these grants from native/legacy
  `UnofficialTicket` values so the server selects the correct validation path.
- Client HTTP runs on the shared Tokio runtime. Travel resumes on the game
  thread only if its epoch, weak controller and original world are still valid.
- Server delegates are cloned, stored and destroyed on the game thread. Workers
  carry completion IDs, not Unreal pointers. At most 64 admissions are pending.
- Restored inactive PlayerStates must redeem a fresh ticket on a new connection.
  Their cached PlayFab proof is invalidated through the native ID-wrapper API;
  the original initialization then performs confirmation again. Weak connection
  witnesses prevent delayed completions from authenticating another connection.
  Already-verified connections retain native behavior through server travel.
- Registration keys remain private to the SDK lifecycle owner. Redemption is
  serialized with registration updates/deletion and has no automatic retry.
- Native confirmation uses a build-specific verified pattern and delegate ABI.
  The tested executable SHA-256 is
  `d82b5eaa122aaee15366499bbce34e325741fecd06d6b7b5490fc650b5afb48b`.
  An unavailable adapter rejects opted-in joins rather than guessing offsets.

## Validation

See the [runtime validation ledger](player-identity-runtime-validation.md) for
two-client replication, map travel, the saved-DLL smoke test and remaining gaps.

Current public-proof flow, 2026-11-16:

- Distinct Steam and Heroic EGS logins read one another's temporary public keys;
  writes and cleanup succeeded in both directions.
- Native browser admissions succeeded for Epic with a Steam verifier and Steam
  with an Epic verifier. Both redeemed grants, reached valid base/PlayFab/platform
  IDs, and confirmed reserved-key cleanup. No player bearer was accepted by the
  challenge/issuance API.
- Live service rejected challenge and grant replay. Wrong completion secrets and
  absent public proofs returned 401; the old bearer request returned 400.
- Steam process-restart reconnect passed after native logout of the previous
  connection, with fresh redemption and cleanup. An earlier immediate attempt
  did not complete admission despite successful proof issuance/cleanup; cause
  unconfirmed. Normal UI leave/rejoin and current-flow map travel remain untested.
- Backend: 662 passed, 4 skipped. Windows/Wine: 480 passed, 20 ignored.
  Host integration: 38 passed. Mypy and generated-output checks pass.
- Evidence: `/tmp/opencode/playfab-investigation/challenge-runtime-evidence.json`
  and `challenge-backend-evidence.json`. Runtime diagnostics/verifier provisioning
  are scratch-only, not shipped. Microsoft remains untested.

Earlier bearer-forwarding prototype evidence (not a current-flow retest):

A native Steam browser join used automatic SDK registration, real PlayFab
verification and server redemption. The native setter assigned a valid PlayFab
ID; the joined client observed valid base, PlayFab and platform IDs. This fixes
the reproduced missing native session-authentication-entry failure without
fabricating session state. Scratch UI automation is not shipped.

Fresh browser reconnects on the same live server each redeemed a new ticket;
live replay was rejected. A server travel back to Courtyard reached InProgress
with all three client ID fields still valid. Windows/Wine library tests:
479 passed, 20 ignored. Backend tests: 650 passed, 4 skipped.
An unissued ticket was rejected through native completion on both a fresh join
and a restored-state reconnect. A fresh valid browser join recovered afterward.

A second observing client's replicated remote IDs are not yet validated.
Linked Steam IDs are account associations, not current ownership
proof; this flow does not manufacture EOS or Discord identities.

Changes currently span this worktree and the sibling backend worktree
`C2ServerBrowserBackend-session-validation`. Until that backend revision is
committed and pinned in `Cargo.toml`, local builds need Cargo patches for
`server-browser-client`, `server-browser-client-sdk` and `server-browser-models`
pointing to the sibling backend's `sdk/rust` crates. No generated SDK code is
copied into the plugin. Pass `--config .cargo/game-identity-local.toml` to Cargo
for this sibling-worktree layout, for example:

```sh
cargo --config .cargo/game-identity-local.toml test -p plugin-backend-integration-tests
```
