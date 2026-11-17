# Credential-free PlayFab identity validation investigation

Investigated and implemented 2026-11-16. **The public-data challenge now replaces
the bearer-forwarding prototype.** Native Steam and Heroic EGS browser joins
passed with opposite-storefront verifier accounts. Photon remains an unvalidated
alternative; this custom challenge is not a native PlayFab authentication protocol.

For native behavior rather than replacement designs, see the
[base-game authentication report](base-game-player-authentication.md).

Initial probes were read-only; later user-approved probes published and removed
temporary public keys. Joining-player credentials go directly from the game to
the fixed PlayFab HTTPS host, not through our backend. The independent verifier
uses its own session; the live test explicitly provisioned it in backend memory
through a scratch-only loopback helper. No account creation, title configuration
changes or raw credential/response capture.

## Implemented protocol

See the [runtime validation ledger](player-identity-runtime-validation.md) for
subsequent replication, map-travel and uninstrumented smoke results, including
unresolved UI/reconnect checks.

- Backend OpenAPI-first operations begin a challenge and complete it for a
  server-bound single-use grant; generated Python/Rust clients, models and
  interfaces are updated. The bearer request field is removed and rejected.
- The client obtains its own account directly from PlayFab and checks it matches
  native join options. It creates a random 256-bit private completion secret and
  sends only its SHA-256 commitment when beginning the challenge.
- Both sides compute a domain-separated proof hash binding the backend audience,
  server ID, uppercase account ID, fresh challenge ID and secret commitment.
  The backend also binds its challenge record to the live registration generation,
  caller IP and 60-second expiry.
- The client publishes only that hash with public permission to the fixed
  `UnchainedJoin_v1` key. Backends cannot choose arbitrary keys or user-data values.
- The backend reads the exact account/key through PlayFab with its independent
  verifier session. Completion requires the private secret and a matching public
  proof; it consumes the challenge before minting a grant. Seeing the public hash
  is not sufficient. Game-thread native completion/setters remain unchanged.
- Cleanup is attempted and checked on success or failure. Only a matching current
  proof is removed. Prior protocol-shaped leftovers can be replaced; unrelated
  data blocks the join. One fixed key bounds crash leftovers.

See [configuration and validation](player-platform-ids.md). Backend configuration
requires `GAME_JOIN_TICKET_DATABASE`, exact canonical `GAME_JOIN_AUDIENCE`, and
`GAME_JOIN_VERIFIER_SESSION_TICKET`. Operators must legitimately provision and
renew that independent session. No automatic login, account creation, token
extraction or refresh service is shipped. Expired verifier sessions fail closed.

Concurrent joins can fail closed because PlayFab offers no compare-and-swap for
the key. Crashes/network failures may leave a public hash. Title API quotas,
policy, latency, verifier-session operations and independent security review
remain deployment concerns. Account control is not a separate proof of current
storefront entitlement or every Torn Banner ban/admission rule. Microsoft login
and second-client observation of remote replicated IDs remain untested.

### Live validation

- Distinct Steam and Heroic EGS accounts published and read each other's public
  keys in both directions. Looking for the peer's key on the wrong account found
  no value. Both temporary keys were removed and absence verified.
- Epic joined through the native browser with the Steam account as verifier;
  Steam joined with the Epic account as verifier. Both completed real PlayFab
  public-proof reads, redeemed grants, reached valid base/PlayFab/platform IDs,
  and confirmed reserved-key cleanup.
- Live service rejected reuse of both completed challenges and redeemed grants.
  Wrong completion secrets and absent public proofs returned 401. The removed
  player-bearer request shape returned 400.
- Steam process-restart reconnect passed after native logout of the previous
  connection. An earlier immediate attempt did not complete admission despite
  successful proof issuance/cleanup; its exact cause remains unconfirmed.
- Backend: 662 passed, 4 skipped; plugin Windows/Wine: 480 passed, 20 ignored;
  host integration: 38 passed. Mypy and generated-output checks pass.
- Tests use isolated launch/injection and scratch-only UI automation/provisioning,
  not console direct travel. Installed plugin binaries were not replaced.

## Authoritative references

PlayFab publishes **Swagger 2.0** contracts and its SDK-generator JSON inputs in
[PlayFab/API_Specs][specs]. Reviewed all 20 `Swagger/PlayFab` files (762 paths)
at revision `80e104aaec1278bd184843809d40344fb129db20`, API version `260922`.
The repository [README][spec-readme] confirms these definitions drive public SDK
generation. Pin these upstream definitions rather than inferring security from
endpoint names or writing an independent account API contract.

Cross-checked public SDK implementations:

- [XPlatCppSdk][cpp], revision `06d850bb770752726cfdbe4a0b2be08df5d59d47`:
  client/server/authentication source, including actual authorization headers.
- [PythonSdk][python], revision `83aaa8e586a7cec9f30630250bbb2c6f43b208a2`:
  corroborates the headers; older than the current schemas. Its legacy
  Matchmaker `AuthUser` requires `X-SecretKey`, not an anonymous validator.

Do not read `GetEntityToken.security = None` as anonymous access: its operation
details require an existing credential, and [the C++ SDK][entity-sdk] selects
`X-EntityToken`, `X-Authorization` or `X-SecretKey`. This matters when generating
clients from the schema. The SDK uses `X-Authorization` for the session-ticket
case, despite the operation-details text referring to `X-Authentication`.

## Candidate mechanisms

| Mechanism | What it establishes | Constraint |
| --- | --- | --- |
| [`Server/AuthenticateSessionTicket`][session] | Session owner and expiry | Requires the title secret; also receives the player's full ticket. Not a privacy fix. |
| [`Authentication/ValidateEntityToken`][entity] | Entity identity, lineage, provider, revocation/expiry | Caller must be the **title** entity. A player entity token cannot call it. Forwarding a player's entity token still exposes a bearer credential. |
| [`Client/GetTitlePublicKey`][public-key] | Key for encrypting account-creation requests | Not a published token-verification key or JWKS endpoint; also requires `TitleSharedSecret` in the request. |
| [`Client/GetPhotonAuthenticationToken`][photon-token] | A narrower credential for Photon authentication | Requires a Photon application associated with the title. Worth investigating; does not require forwarding the full PlayFab session to our backend. |
| [`Client/UpdateUserData`][write-data] + [`Client/GetUserData`][read-data] | Ability to publish a challenge on the claimed account | Implemented and tested on distinct Steam/EGS accounts after write approval; requires an independent verifier login and mutates one reserved key. |
| [`Client/GetPlayFabIDsFromSteamIDs`][steam-map] + Steam authentication | Authenticated Steam ID mapped to a PlayFab account | Lookup alone is not proof. Needs a real Steam ownership proof and an independent PlayFab caller credential. Steam-only unless another platform has an equivalent mapping. |
| [`Client/ExecuteCloudScript`][cloudscript] | Could invoke a title-provided proof issuer | Cannot install our own handler or signing key in Torn Banner's title; no suitable existing handler identified. |

No offline session/entity-token verification contract or appropriate signing-key
discovery operation was found in the reviewed schemas/SDKs. That is a bounded
finding, not proof that PlayFab has no other private or future mechanism.

## Photon: narrower bearer credential

The [official PlayFab quickstart][photon-guide] and [Photon documentation][photon]
describe this sequence:

1. Client calls PlayFab `GetPhotonAuthenticationToken` using its own session,
   directly over HTTPS, with the title-associated `PhotonApplicationId`.
2. It supplies only the returned `PhotonCustomAuthenticationToken` and PlayFab ID
   to the verifier.
3. The verifier calls `https://{title}.playfabapi.com/photon/authenticate` with
   `username` and `token`; it must use the authoritative success identity, not
   accept the submitted username as proof by itself.

This avoids exposing the **full PlayFab session**. It still exposes a scoped
bearer token: do not assume it is server-bound, single-use, non-replayable or
short-lived without validating its actual semantics. The public quickstart
does not establish those guarantees for our join design. Keep our own admission
challenge, expiry, server binding and replay protection.

Live checks for title `EBF8D`:

- Anonymous Photon validation with a synthetic invalid token: HTTP 200,
  `resultCode: 2`, `message: Invalid token`. HTTP success is not auth success.
- Authenticated `GetPhotonAuthenticationToken` with the all-zero test application
  ID: HTTP 400, `PhotonApplicationNotAssociatedWithTitle`.
- This does **not** prove Photon is disabled. It proves that the test application
  is unusable. No legitimate associated application ID was identified, and no
  successful Photon token issuance/validation was demonstrated.
- Associating our own Photon application requires title-owner configuration;
  it cannot be done just by owning a Photon account.

## Public user-data challenge: no player bearer at the verifier

The contract provides a different route to proving account control:

- `UpdateUserData` operates on the authenticated caller, has **no target
  PlayFabId field**, and can publish specific keys with `Permission: Public`.
- `GetUserData` accepts another player's `PlayFabId` and returns only that
  account's public keys. The verifier can use **its own** session, not the
  claimant's session. A title secret is not required by this client operation.

The implemented protocol follows this design:

1. Backend creates a bounded pending challenge bound to its origin, claimed
   PlayFab ID, current server registration/generation and expiry.
2. Client retains a private completion secret and publishes only a
   domain-separated commitment to that secret and challenge through PlayFab,
   using its session directly. Never accept arbitrary user-data keys from a
   backend response; the plugin must constrain the key and payload.
3. An independently authenticated verifier fetches that exact public key from
   the claimed account directly from PlayFab and checks the commitment.
4. Completion also requires the private secret, then atomically consumes the
   challenge and issues the usual single-use server-bound grant.

The public value must **not** itself be a redeemable secret: another logged-in
player can read it. Include protection against challenge theft, login-CSRF/relay,
cross-backend reuse, stale values, concurrent joins and account-ID substitution.
Do not put server keys, raw sessions, redeemable grants or private join details
in public player data.

After explicit user approval, a live single-account probe confirmed:

- The unique namespaced probe key was absent before writing.
- `UpdateUserData` with `Permission: Public` returned HTTP 200.
- `GetUserData` returned the exact test value with `Permission: Public`.
- `UpdateUserData` with `KeysToRemove` returned HTTP 200, and a final read
  confirmed the key was absent.
- Only that key was changed; other keys and permissions were not submitted.
  The test value was not a credential, challenge secret or redeemable grant.

That initial probe established only own-account behavior. Subsequent distinct
Steam/EGS cross-account reads and browser admissions passed as recorded above;
Microsoft remains untested.

Deployment concerns:

- Provision and renew a legitimate independent verifier login. Do not quietly
  reuse players' tokens, create accounts in the title, or assume custom-ID login
  is enabled. A server-owned in-process login is another architecture, but its
  reports do not automatically become trusted central-backend proof.
- Assess title API policy, rate/billing limits, eventual consistency, collisions,
  privacy and operational acceptability. This is not a ready-made PlayFab auth
  protocol and deserves independent security review before production deployment.

## Steam proof plus PlayFab mapping

The mapping API returns PlayFab IDs for linked Steam IDs using the **caller's**
session. Therefore a real Steam ownership proof plus an independently performed
mapping lookup can establish a Steam-to-PlayFab association without receiving
the claimant's PlayFab bearer. My earlier statement that Steam proof could not
establish the claimed PlayFab ID was incomplete: it needs this additional trusted
mapping step.

Live `GetPlayFabIDsFromSteamIDs` for the signed-in account's own Steam ID returned
HTTP 200 and the expected PlayFab ID. Only a match boolean was saved. This
confirms that the operation works for title `EBF8D`; a distinct verifier account
and a real Steam ownership-proof flow remain untested. The claimant's own lookup
was only a capability test, not evidence sufficient to authenticate a join.

Do not use the claimed platform RPC ID or a mapping response supplied by the
joining client as ownership proof. Verify the platform ticket/OpenID assertion
through the platform's supported verifier, then look up the verified ID directly.
Test account relinking and title-policy behavior. This proves a linked account
association, not that a particular existing PlayFab session is current, and it
does not cover Epic/EOS automatically.

## Chivalry's native ticket is a different API

Neither `/GetJoinTicket` nor `/ConfirmJoinTicket` appears in the reviewed public
PlayFab specs. The current executable contains generated Torn Banner API clients:

- Client base: `https://v9.c.api.c2.tornbanner.io`.
- Server base: `https://v10.s.api.c2.tornbanner.io`.
- Confirmation request `0x141370340` adds `/ConfirmJoinTicket`.
- Common request preparation `0x1413addc0` sends header **`Token`**, taken from
  request `+0xd8`; it is not PlayFab's `X-Authorization` session-ticket API.
- Caller `0x1414f0c70` copies that token from the session-auth map at session
  interface `+0x728` and exits before HTTP when the entry is missing.
- Serializer `0x1413bf900` emits `PlayFabId`, `Ticket`, `EosIdentity`.
- An anonymous request to the exact server endpoint with synthetic invalid IDs
  and ticket returned HTTP 401. No valid official server credential was used.

The previous native-browser evidence established a named session but no auth-map
entry. Supplying a fabricated map entry does not grant official authorization.
The old PDB-equipped build does not expose the new join-ticket functions, so
generic old registration symbols are not evidence of their current auth policy.

Native official confirmation remains worth pursuing **if** we can obtain a
legitimate server-scoped `Token` for a user-hosted session. PlayFab's public SDKs
do not supply that Torn Banner credential. No supported public issuance path
for it has been established; no attempt was made to obtain title secrets or
impersonate an official host. Restoring native confirmation remains unproven.

## Historical recommendation and evidence

This section records the earlier opaque-grant investigation. The current plugin
uses signed offline admissions and persistent first-use account pinning; see
[current runtime status](player-identity-runtime-validation.md). Microsoft Store
support is outside the current scope. The plugin no longer issues or redeems
opaque grants; backend compatibility endpoints remain available separately.

The primary design must use PlayFab and cover Epic Games Store and potentially
Microsoft, not depend on Steam ownership. The public-data challenge now satisfies
the tested Steam/EGS admission path using an independent verifier login.
Steam mapping is supplementary investigation, not the primary architecture.
Also ask the title owner whether a scoped native ticket verifier or associated
Photon application is available. Prefer a supported scoped-ticket flow if one
can be legitimately used.

Read-only runtime results already confirmed player-entity self-validation is
rejected: `GetEntityToken` returned a `title_player_account`; using it to call
`ValidateEntityToken` returned HTTP 401, `NotAuthorized`.

Scratch evidence: `/tmp/opencode/playfab-investigation/` contains the pinned
download manifest, operation/security summary, schema/SDK copies and sanitized
`live-capabilities.json`, `native-negative-check.json` and
`public-data-write.json`. Cross-storefront probes are in `cross-*-evidence.json`;
native admissions and negative checks are in `challenge-runtime-evidence.json`
and `challenge-backend-evidence.json`. These record sanitized statuses/booleans,
not raw credentials or account responses. Scratch diagnostics/provisioning are
not shipped; the installed plugin is unchanged.

An uninstrumented release artifact is saved at
`/tmp/opencode/playfab-investigation/UnchainedPlugin-credential-free.dll`, with
SHA-256 and scope in `challenge-production-artifact.json`. It was not installed;
a subsequent Steam client/server smoke test used this exact uninstrumented
artifact. Epic's runtime evidence for that run uses the diagnostic build.
Games/backend were stopped and the prior scratch DLL restored. Implementation
is now committed and pushed on both feature branches, with an immutable SDK
revision in `Cargo.toml`; local SDK overrides are no longer required.

[specs]: https://github.com/PlayFab/API_Specs/tree/80e104aaec1278bd184843809d40344fb129db20/Swagger/PlayFab
[spec-readme]: https://github.com/PlayFab/API_Specs/blob/80e104aaec1278bd184843809d40344fb129db20/README.md
[cpp]: https://github.com/PlayFab/XPlatCppSdk/tree/06d850bb770752726cfdbe4a0b2be08df5d59d47/code/source/playfab
[python]: https://github.com/PlayFab/PythonSdk/tree/83aaa8e586a7cec9f30630250bbb2c6f43b208a2/playfab
[entity-sdk]: https://github.com/PlayFab/XPlatCppSdk/blob/06d850bb770752726cfdbe4a0b2be08df5d59d47/code/source/playfab/PlayFabAuthenticationApi.cpp#L131-L225
[session]: https://github.com/PlayFab/API_Specs/blob/80e104aaec1278bd184843809d40344fb129db20/Swagger/PlayFab/Server.swagger.json#L283
[entity]: https://github.com/PlayFab/API_Specs/blob/80e104aaec1278bd184843809d40344fb129db20/Swagger/PlayFab/Authentication.swagger.json#L62
[public-key]: https://github.com/PlayFab/API_Specs/blob/80e104aaec1278bd184843809d40344fb129db20/Swagger/PlayFab/Client.swagger.json#L2579
[photon-token]: https://github.com/PlayFab/API_Specs/blob/80e104aaec1278bd184843809d40344fb129db20/Swagger/PlayFab/Client.swagger.json#L1467
[write-data]: https://github.com/PlayFab/API_Specs/blob/80e104aaec1278bd184843809d40344fb129db20/Swagger/PlayFab/Client.swagger.json#L6345
[read-data]: https://github.com/PlayFab/API_Specs/blob/80e104aaec1278bd184843809d40344fb129db20/Swagger/PlayFab/Client.swagger.json#L2655
[steam-map]: https://github.com/PlayFab/API_Specs/blob/80e104aaec1278bd184843809d40344fb129db20/Swagger/PlayFab/Client.swagger.json#L2217
[cloudscript]: https://github.com/PlayFab/API_Specs/blob/80e104aaec1278bd184843809d40344fb129db20/Swagger/PlayFab/Client.swagger.json#L719
[photon-guide]: https://learn.microsoft.com/en-us/xbox/playfab/live-service-management/service-gateway/add-ons/photon/quickstart
[photon]: https://doc.photonengine.com/server/v5/reference/playfab
