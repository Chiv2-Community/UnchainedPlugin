# Validated identity runtime checks

2026-11-16. **Runtime validation is incomplete; this is not merge approval.**
Implementation is being committed/pushed on the feature branches; no merge or
installed-plugin replacement is authorized by these validation results.

## EOS Connect proof retrieval

Isolated Steam and Epic runs retrieved identity-only JWTs through the public
`EOS_Connect_CopyIdToken` API and verified them against Epic's public JWKS.
Both verified PUIDs matched prior native client/host observations. Steam's signed
external identity matched the controlled Steam account; Epic's verified platform
identity used its EOS PUID. No raw JWTs or reusable credentials were saved.

- Exact issuer: `https://api.epicgames.dev/auth/v1/oauth`.
- Both games used the same native EOS client/product/sandbox/deployment settings.
- Redpoint delay-loads the SDK: the adapter must load its exact game-relative DLL
  before capturing platform creation, rather than require it to be loaded already.
- Evidence: `/tmp/opencode/eos-token-live/result.json` and private probe sources.
- This establishes EOS account proof, not PlayFab/EOS account linkage or ownership.
- Native typed-ID construction passed with validity and exact string round-trip
  checks: PlayFab and `RedpointEOS` on both storefronts, plus Steam on Steam.
  The FString constructor is vtable slot 32 (`+0x100`), as also used by native
  PlayerState initialization; declaration order incorrectly suggested slot 33.
  The `EOS` subsystem name does not expose the required interface. These tests
  construct wrappers only; they do not establish authoritative field assignment.

Registration-bound cached-JWKS state and conservative lease deadlines compile for
Windows. Host-state tests cover preservation across pause/expiry/renewal. The
opted-in native path now requests signed admissions and verifies/consumes them
synchronously without online redemption. Native base/PlayFab/platform setters
receive wrappers constructed from verified claims. The platform RPC's server
implementation drops client updates on opted-in hosts; normal replication stays
unchanged. Live validation of this integration is in progress; this is not yet
a runtime pass. Legacy backend operations remain, but are never a rejection fallback.

## Passed

- Native Steam and Heroic EGS browser joins with opposite-storefront independent
  verifier accounts (earlier challenge-flow run).
- Two simultaneous clients: Steam's local player `259` appeared remotely on Epic
  with valid base/PlayFab/platform wrappers; Epic's local player `258` appeared
  remotely on Steam with the same three valid wrappers. Observations used each
  client's current world's GameState PlayerArray on the game thread, not an
  arbitrary scan of old PlayerStates.
- `servertravel FFA_Courtyard`: both clients retained valid local and remote
  wrappers. The server reached `InProgress` at 10:13:57 after travel at 10:12:53
  (local timestamps). A Steam snapshot during loading found no world; a later
  snapshot found the loaded world and both players.
- Uninstrumented Steam client **and server** smoke test through the native browser
  using external mouse automation, not console travel. DLL SHA-256:
  `49c6471e267db8353838689e57d588dc091117a084959430fc92ea7c87c156ee`.
  Client logged `grant=true cleanup_verified=true`; server logged native admission
  `accepted=true` at 10:20:49. Both runs lacked scratch identity diagnostic hooks.
  Gameplay was visible in Courtyard.

Snapshots also contain the server's local bootstrap player `257`, whose PlayFab
wrapper is unset. It is not either network client; peer checks above use their
locally observed player IDs. These checks establish wrapper validity/replication,
not raw account-string equality.

The smoke backend retained the explicitly provisioned Epic verifier session in
memory across game restarts. Scratch composition/provisioning surrounded the
production challenge/grant service; this is not a provisioning solution or an
unmodified production backend deployment.

## Still open

- **Normal UI leave/rejoin:** Steam's actual in-match Match and Settings menus
  displayed `Exit Game`, not a return-to-menu option. Earlier scratch invocation
  of `LeaveSessionButton` terminated the client. Neither establishes successful
  ordinary disconnect/rejoin, or proves the proof protocol caused the behavior.
  Establish the intended return-to-menu path and test it on both storefronts.
  Subsequent user-operated Epic leave/rejoin passed as recorded below; Steam
  returned to Frontend but its same-process rejoin remains pending.
- **Rapid reconnect:** the earlier immediate Steam process-restart attempt
  issued/cleaned a proof but never completed native redemption. Waiting for the
  old connection's native logout allowed a fresh join. This run did not resolve
  or newly reproduce that failure; delayed reconnect is not an immediate-reconnect
  pass.
- **Uninstrumented Epic smoke:** Epic admission/remote replication passed with
  scratch diagnostics, not the saved uninstrumented DLL.

No main-plugin behavior was changed for these tests. Observation/UI helpers stay
outside the repository. Installed plugin binaries were not replaced.

## Invalid-proof browser tests

Subsequent live tests passed all four cases on **both Steam and Epic** against
the saved uninstrumented opted-in server DLL:

| Client behavior | Observed result |
| --- | --- |
| Omit public proof | Independent PlayFab read rejected; no grant; browser travel cancelled. |
| Publish a one-character-mismatched proof hash | Independent PlayFab read rejected; no grant; browser travel cancelled. |
| Submit the wrong private completion secret | Completion rejected before proof verification; no grant; browser travel cancelled. |
| After mismatched-proof rejection, bypass client cancellation with a correctly shaped but unissued grant | Backend redemption rejected; native server completion logged `accepted=false`; client returned to Frontend. |

Every rejected case confirmed reserved-key cleanup and a post-rejection Frontend
world. No case issued a real grant or logged successful server admission. Both
platforms subsequently joined with valid proof and all three local wrappers
valid. Client tampering is scratch-only; server enforcement was unmodified.

Each case and the valid recovery used a fresh client process. An initial attempt
to retry from the same browser after cancelled travel invoked the buttons but
never reached the next proof request. The cause remains unconfirmed; do not
count fresh-process recovery as proof that same-process retry works.

Evidence: `/tmp/opencode/playfab-investigation/negative-browser-evidence.json`;
initial same-process retry evidence is `retry-limitation-negative-browser-evidence.json`
in that directory. Both clients/server/backend were then left running, with
tampering disabled, pending user-operated UI leave/rejoin; this supersedes the
stopped-process state recorded for the earlier run below.

## User-operated UI rejoin

The user reported successful rejoin; logs confirm **Epic** kept the same process
(`424`) through Frontend at 11:00:17, fresh proof/grant with cleanup at 11:00:30,
server acceptance at 11:00:35, and Courtyard PlayerState with all three wrappers
valid at 11:00:37. This establishes same-process UI leave/rejoin for Epic, not
immediate reconnect after an abruptly terminated process.

Steam also returned to Frontend at 10:56:58 in its existing process (`3420`), but
had no subsequent grant/admission at the time of inspection. Its UI rejoin is
still pending. Earlier menu-label observations do not establish that returning
to Frontend is impossible. Exact user menu actions were not captured.

Evidence: `/tmp/opencode/playfab-investigation/manual-rejoin-log-snapshot.json`.
Games/backend remain running pending the remaining user-operated check.

## Native ListPlayers formatting defect

The user's list showed both real clients' correct PlayFab IDs, a duplicate local
server player with `NULL`, and the same negative integer under `EOSPlayerId`.
The controlled Steam/Epic account IDs matched the two non-NULL rows.

Static inspection of current Steam executable `ListPlayers` at `0x1419eb110`
confirmed a display defect. At `0x1419eb359` its formatter receives:

```text
name, PlayFab-ID string, base-unique-ID string, score, kills, deaths, ping
```

But the format at `0x144759aa0` is:

```text
%s - %s - %d - %d - %d - %d ms\n
```

The missing third `%s` renders the low 32 bits of the base-ID string pointer as
the displayed integer; subsequent statistics shift left and ping is not consumed.
It reads the base wrapper at `+0x288` and PlayFab wrapper at `+0x5c8`, not the
separate platform wrapper at `+0x5f0`. Thus this output cannot establish that the
actual EOS identity is missing or invalid, or that the base identity is an EOS
PUID. No formatting hook or identity change has yet been implemented for this
finding. The earlier bootstrap-player observations explain the duplicate NULL
row; it is separate from the authenticated Steam network client.

## EOS identity contents and replication

Read-only inspection on 2026-11-16 at 17:25 UTC compared current GameState
PlayerArray entries on the uninstrumented server and both running clients.
Bounded Windows `ReadProcessMemory` reads inspected native cached ID contents;
no game memory was changed and no credentials were read. Layouts and string
representations are specific to the tested executable builds.

- Steam network player `263` and Epic network player `260` had distinct
  32-character hexadecimal RedpointEOS product-user-ID strings in their base
  wrappers. Each account's value matched exactly on its own client, the server,
  and the other client.
- Epic's platform wrapper contained the same EOS PUID in all three views.
- Steam's platform wrapper was a native Steam numeric ID on Steam/server and
  a generic string on Epic. The latter retained a leading zero; numeric values
  agreed. The Steam ID is separate from the Steam account's EOS PUID.
- Bootstrap player `257` shared the host Steam account's EOS/platform IDs but
  had no PlayFab wrapper. It was distinct from authenticated network player
  `263`; this explains the duplicate `NULL` row.

These checks establish distinct ID contents and consistent replication, not
independent EOS Connect account ownership or protection against a malicious
client submitting another account's EOS ID. The PlayFab account-control proof
must not be described as independent EOS-PUID authentication.

The scratch reader used native RedpointEOS cached UTF-8 strings, PlayFab/generic
UTF-16 strings, and the Steam `uint64` representation. Static inspection confirmed
Steam's ToString formats the value at `+0x18` with `%llu`; generic ToString copies
the FString at `+0x18`. Unsupported layouts fail or remain explicitly undecoded.
External snapshots are not atomic with travel; an initial stale PlayerState read
failed, and final checks reacquired the current pointers before comparison.

Evidence: `/tmp/opencode/playfab-investigation/eos-identity-validation.json`;
reproduction script: `validate_eos_ids.py` in that directory. All cross-view
assertions passed. Games/backend were left running; no production plugin changes
or `ListPlayers` formatting fix were applied for this check.

## Confirmed client-controlled platform-ID update

Isolated runtime tests on 2026-11-16 reproduced a post-admission platform-ID
spoof twice against the uninstrumented server DLL above. Both accounts first
joined normally using valid PlayFab proofs and independently provisioned verifier
accounts. A scratch Epic client then submitted the controlled Steam account's
base EOS ID through its own platform-ID RPC.

- The server replaced Epic's platform field with Steam's EOS ID.
- The false value replicated to both clients, including the owning client.
- Epic's base EOS ID and verified PlayFab ID did not change.
- A later valid Epic join restored its legitimate platform ID after waiting
  for the previous connection's native logout. An earlier reconnect attempted
  before logout timed out after issuance without redemption.

This proves the platform update is client-controlled despite validated PlayFab
admission. It does **not** prove the base EOS admission ID is spoofable, nor that
the PlayFab proof was bypassed. Server-to-client replication itself is not the
trust-boundary defect; accepting client-provided identity updates is.

The final run used private Wine prefixes, source/build outputs, backend ports
18093/18094 and game port 7787. Private test processes were stopped; other agents'
processes and caches were not changed during that run. No production fix was
applied. Join-ticket redemption currently returns only PlayFab validation, not
an independently verified EOS/platform identity; server-authoritative platform
assignment requires additional verified identity data, not a signed client claim.

Evidence: `/tmp/opencode/eos-spoof-isolated/control/result.json`; earlier
reproduction: `confirmed-platform-spoof.json` in that directory.

## Evidence

Under `/tmp/opencode/playfab-investigation/`:

- `extended-initial-snapshots.json`: both clients' GameState observations.
- `steam-post-travel-ready.json`, `epic-post-travel.json`: post-travel observations.
- `extended-travel-server.json`: travel, native initialization and match states.
- `extended-challenge-runtime-evidence.json`,
  `extended-challenge-backend-evidence.json`: diagnostic admissions/verifier separation.
- `steam-menu-exit-game.png`: observed in-match menu.
- `production-smoke-evidence.json`, `production-steam-gameplay.png`: artifact
  hashes, absence of diagnostic hooks, cleanup, native admission and gameplay.

These files contain no credentials or raw account responses. Games/backend were
stopped and the prior scratch DLL restored after validation.
