# Base-game player authentication and validation

Report date: 2026-11-16. Scope: Chivalry 2's native executable, **without an
Unchained replacement authentication protocol**.

## Summary

The game separates account login, connection admission and player identity
initialization. A successful connection does not by itself establish a verified
PlayFab identity.

- The client has a signed-in PlayFab account and a session bearer credential.
- Native browser travel carries a claimed `PlayFabId` and, conditionally, an
  `UnofficialTicket` obtained from the signed-in account's attributes.
- Server initialization distinguishes official/matchmade sessions from the
  unofficial-ticket path. Not every join calls `/ConfirmJoinTicket`.
- In the unofficial path, the server asks Torn Banner's service to confirm
  `PlayFabId`, `Ticket` and `EosIdentity`, using a separate server/session `Token`.
- A successful asynchronous confirmation leads to native typed PlayFab-ID
  assignment. Platform-ID assignment is a separate client-to-server RPC.
- Base Unreal ID, PlayFab ID and platform ID are distinct fields. Their presence
  and validity are not interchangeable authentication guarantees.

The client/server control flow is recoverable from the executable. The service's
internal ticket checks, official-host credential provisioning and all platform
login variants have **not** been reconstructed end to end.

## Evidence and limits

Evidence labels used below:

- **Static:** inspected shipping-executable control flow, strings and serializers.
- **Runtime:** observed native functions through diagnostic instrumentation.
- **Reference:** public PlayFab contracts/SDKs or local Unreal source; not proof
  that every game path uses that exact reference implementation.
- **Unknown:** not established by this investigation.

Runtime evidence comes from instrumented Steam clients joining user-hosted
servers through a redirected browser. Those tests help explain native function
behavior, but are **not tests of an entirely unmodified game joining an official
server**. Results from the later plugin-issued ticket implementation are not
used here as evidence of stock authentication.

All addresses below refer to the current Steam executable at image base
`0x140000000`, SHA-256:

```text
d82b5eaa122aaee15366499bbce34e325741fecd06d6b7b5490fc650b5afb48b
```

Addresses are analysis landmarks, not a stable ABI. Current function names are
descriptive unless corroborated by reflection or embedded source strings. The
older PDB-equipped executable differs materially and cannot establish current
join-ticket behavior.

## 1. Identity and credential types

| Item | Role | What it does not establish alone |
| --- | --- | --- |
| PlayFab ID | Account identifier within the game's PlayFab title | Possession of the account or a current authenticated session |
| PlayFab session ticket | Bearer credential for permitted PlayFab client APIs | A server-bound, single-use join authorization |
| Native `UnofficialTicket` | Account attribute copied into join options for the unofficial flow | Its expiry, replay policy or cryptographic format are unknown |
| Torn Banner `Token` | Header authorizing the server/session's confirmation request | Not the joining player's PlayFab session ticket |
| Base Unreal unique ID | Typed connection/session identity in `APlayerState` | Completion of Chivalry's separate PlayFab confirmation |
| Platform unique ID | Additional identity supplied through the platform subsystem/RPC | Independent proof of PlayFab account ownership |
| `EosIdentity` | Named field in native join confirmation | Not proven to mean any arbitrary Steam, Epic or Microsoft ID |

Treat identifiers as claims until the corresponding trusted validation step has
succeeded. A structurally valid `FUniqueNetId` is not necessarily authenticated.

## 2. Account login

**Static:** the executable includes PlayFab online-subsystem identity handling
and Torn Banner API operations named `/LoginPlayFab` and `/LoginEOS`. The native
PlayFab identity exposes the local account ID, account attributes and auth token.

**Runtime:** an existing Steam login produced a credential accepted by
`https://EBF8D.playfabapi.com/Client/GetAccountInfo` with an empty request body and
header `X-Authorization`. PlayFab returned the authenticated account and a linked
Steam association. This establishes that the recovered value is usable as a
PlayFab client bearer credential; it is not merely a display ID or join ticket.

**Reference:** PlayFab publishes platform login operations such as
[`Client/LoginWithSteam`][login-steam], and its [client SDK][client-sdk] sends
session tickets in `X-Authorization` for authenticated account operations.
These contracts describe PlayFab capabilities, not proof that Chivalry directly
calls that login operation in every storefront flow. Its Torn Banner login
operations are a separate API layer.

**Unknown:** the complete platform-ticket exchange, Torn Banner login request
and response contracts, refresh/logout behavior, and the code that first populates
the `UnofficialTicket` account attribute. Epic/EOS and Microsoft login sequences
have not been runtime-tested. A linked platform account is not proof of current
platform authentication.

## 3. Browser selection and connection

**Static:** the native join-options builder at `0x141998e40`, containing the
source label `UTBLLocalPlayer::GetJoinURL`, obtains the local PlayFab identity.
It formats `?PlayFabId=...` and conditionally appends `?UnofficialTicket=...`.
The ticket comes from the signed-in PlayFab user account's attributes.

The session-travel caller is `0x141941d50`. The observed browser flow executes
the native builder before travel. A plain console/direct-travel path must not
be assumed to perform the same preparation.

**Reference:** Unreal's `UPendingNetGame` collects login options and a preferred
typed unique ID, then transmits them separately in `NMT_Login`. The options and
the connection's unique ID are therefore different inputs.

Neither a `PlayFabId` URL option nor transport-level connection success proves
account ownership. The shown builder forwards a join ticket, not the full
PlayFab bearer credential. This is a finding about that builder, not an audit
of all credentials exchanged by the game.

The presence of a `/GetJoinTicket` operation elsewhere in the executable does
not establish that it issued the account attribute used by this browser path.
Likewise, a legacy PlayFab Matchmake `Ticket` must not automatically be equated
with `UnofficialTicket`. Their relationship has not been established.

## 4. Server admission and branch selection

Unreal's baseline flow is `PreLogin` → `Login` → `InitNewPlayer` → session player
registration and base-ID assignment. Chivalry adds its own identity work.
Passing an early admission check does not imply that later identity work has
finished.

**Static:** the native server initialization function at `0x14193b120` reads
`PlayFabId` from connection options and checks existing PlayerState identity
state. A helper at `0x141a429c0` selects a distinct branch based on:

- The named `GameSession` setting `OFFICIAL`.
- A cached helper result from `0x141490000`, which searches a process-global
  string for `matchmade` or `matchmaking`.

When that helper is true, this initialization site constructs a typed PlayFab
ID from the claimed string and calls the native setter directly. When false,
and the additional PlayFab ID is not already valid, it reads `UnofficialTicket`
and enters the confirmation setup at `0x141a22590`.

This proves the branches differ. It **does not** prove official sessions have
no authentication: platform/session admission, earlier service checks and
additional setter behavior must also be accounted for. Nor does setting a local
session flag create a legitimate trusted official session. The upstream trust
boundary for the official/matchmade branch remains incompletely traced.

An already-valid PlayFab field can skip this particular confirmation setup.
Restored PlayerStates therefore matter when reasoning about reconnects; do not
assume every initialization invokes fresh confirmation.

## 5. Unofficial join confirmation

The recovered path is:

```text
server initialization                     0x14193b120
  └─ capture player and completion context 0x141a22590
      └─ confirmation helper              0x1414f0c70
          ├─ require named session
          ├─ require session-auth map entry
          └─ submit /ConfirmJoinTicket    0x141370340
              └─ asynchronous completion  0x1414f73f0
                  └─ player callback      0x141a189d0
```

### Before HTTP

**Static:** `0x1414f0c70` obtains the PlayFab session interface and looks up the
named session's authentication entry in a map at session-interface offset
`+0x728`. Missing subsystem/session/authentication prerequisites cause an early
false return before request submission.

The helper's return value means whether it started the operation, **not** whether
the player was authenticated. The reviewed caller does not turn that early
false return into the asynchronous rejection callback.

### Request and service boundary

**Static:** request builder `0x141370340` targets `/ConfirmJoinTicket`. The
serializer at `0x1413bf900` emits:

```json
{"PlayFabId": "<claimed account>", "Ticket": "<join ticket>", "EosIdentity": "<native identity value>"}
```

Common request preparation at `0x1413addc0` supplies header `Token` from the
request's authentication field, populated from the session-auth map entry.
The executable's server API base is `https://v10.s.api.c2.tornbanner.io`; its
client API base is `https://v9.c.api.c2.tornbanner.io`.

This is **Torn Banner's service**, not PlayFab's public
[`Server/AuthenticateSessionTicket`][authenticate-session] endpoint. The latter
requires `X-SecretKey` and accepts a PlayFab session ticket. It is not the contract
recovered from the native join-confirmation request.

An anonymous request to Torn Banner's exact confirmation endpoint using only
synthetic invalid values returned HTTP 401. That confirms an authorization
barrier for that request, not the policy for legitimate official hosts.

**Unknown:** how legitimate servers obtain/renew `Token`; whether user-hosted
servers can obtain one; the service's ticket issuance, identity binding, expiry,
replay, revocation and EOS checks. Do not describe the ticket as signed,
server-bound or single-use without further evidence.

### Completion and assignment

**Static:** HTTP completion `0x1414f73f0` reads a response boolean at `+0x38` and
executes the supplied delegate. Its execution thunk at `0x141a2b660` forwards to
the player callback at `0x141a189d0`.

- Success: checks the captured player context, constructs a typed PlayFab ID
  from the captured account string, and calls `0x141a51ac0`; native follow-up
  initialization also runs. Embedded label: `set_playfab_id_accept_unofficial`.
- Failure: where the game/session objects are available, invokes rejection
  handling with `Invalid unofficial join data..`. Embedded label:
  `set_playfab_id_reject_failed_unofficial`.
- Missing native unique identity has a separate `No uniqueId.` rejection path
  in the completion setup.

An early inability to start HTTP and an asynchronous negative service response
are different outcomes. The observed early-exit case left PlayFab identity
unset; it was not evidence that Torn Banner rejected a submitted ticket.

## 6. PlayerState fields, platform RPC and replication

The current layout has three distinct identity fields:

| Field | Offset | Assignment path |
| --- | --- | --- |
| `APlayerState.unique_id` | `+0x288` | Unreal/session registration and native base-ID setter |
| `ATBLPlayerState.playfab_oss_unique_id` | `+0x5c8` | Typed PlayFab setter `0x141a51ac0` |
| `ATBLPlayerState.platform_oss_unique_id` | `+0x5f0` | Platform RPC and setter `0x141a51970` |

**Static:** the PlayFab setter rejects null/invalid or non-`PlayFab` IDs,
assigns through the native wrapper, marks replication property `0x18` dirty,
and performs additional initialization. Its type check is not itself a remote
account-ownership check. The base-ID setter override at `0x141a53130` preserves
an already-valid base ID.

The separate platform path is:

```text
client initialization 0x1419f1b90
  → ServerSetPlatformOSSUniqueId wrapper 0x141c64ea0
  → server implementation 0x141a00290
  → platform setter 0x141a51970
```

The server implementation obtains the owning controller's PlayerState and
forwards the supplied ID. The setter rejects invalid IDs, assigns the platform
field and marks replication property `0x19` dirty. No independent platform
ownership-verification call was established in this inspected RPC/setter path;
that is not a claim that no other platform/session validation exists.

The client sends this RPC only under local readiness gates, including a local
PlayFab-ID pointer. Its initialization byte at controller `+0x148d` is set even
if the platform lookup fails and no RPC is sent. This is a possible late-identity
failure mode, not the cause demonstrated in the observed Steam join.

Generated reflection declarations include `OnRep_PlayFabOSSUniqueId` and
`OnRep_PlatformOSSUniqueId`. Replication distributes identity state; it does not
independently authenticate it. Generated SDK stub bodies are not executable-game
behavior.

## 7. What runtime observations establish

The instrumented Steam browser joins observed:

1. A signed-in frontend with valid base, PlayFab and platform fields.
2. Native browser travel containing both `PlayFabId` and `UnofficialTicket`.
3. A user-hosted server with a named session but no matching session-auth entry.
4. Native confirmation returning false before HTTP submission.
5. A working platform RPC, but the joined PlayerState having validity
   `[true, false, true]` for base/PlayFab/platform.

Thus a working login, populated join options, admitted connection and platform
RPC can coexist with missing server-side PlayFab identity. That does not prove
the base game's official-server authentication is broken; these hosts lacked
native session authorization in a redirected, instrumented environment.

No fully stock official-server join, Epic join or Microsoft join was captured
for this report. Second-client replication observation and official ticket
replay/expiry behavior remain unverified.

## References and reproducibility

- Current executable: Steam installation,
  `TBL/Binaries/Win64/Chivalry2-Win64-Shipping.exe`, hash above; Binary Ninja
  `view_3` during this investigation.
- Older PDB comparison executable: SHA-256
  `188c99263209244b5d0d1c18003b3e572973b3e93454295fe22089bdc33a40e1`;
  not a source for current join-ticket function names or addresses.
- ArgonSDK revision `d9d9759e40e623a5f7f41d8c58ea8a2988170a26`:
  `Source/TBL/Public/TBLPlayerController.h` and `TBLPlayerState.h`, for reflected
  RPC/replication declarations.
- Local Unreal reference revision `2d1aaeede6780b14c3ef97272e8a68c3429d8231`,
  under `Engine/Source/Runtime/Engine`: `Private/PendingNetGame.cpp`,
  `LocalPlayer.cpp`, `GameModeBase.cpp`, `GameSession.cpp`, `PlayerState.cpp`,
  and `Classes/GameFramework/OnlineReplStructs.h`.
- Sanitized native-path runtime evidence:
  `/tmp/opencode/player-identity-runtime/join-1-evidence.json`,
  `join-2-evidence.json` and `join-4-evidence.json`.
- Sanitized direct-PlayFab capability and native negative-request evidence:
  `/tmp/opencode/playfab-investigation/live-capabilities.json` and
  `native-negative-check.json`.
- [Pinned PlayFab Swagger 2.0 contracts][specs], revision
  `80e104aaec1278bd184843809d40344fb129db20`; [public C++ SDK][client-sdk],
  revision `06d850bb770752726cfdbe4a0b2be08df5d59d47`. Torn Banner's private
  operations must not be inferred from these PlayFab contracts.

Related: [credential-free validation investigation](playfab-credential-free-validation.md)
covers possible replacements, not stock behavior.

[specs]: https://github.com/PlayFab/API_Specs/tree/80e104aaec1278bd184843809d40344fb129db20/Swagger/PlayFab
[login-steam]: https://github.com/PlayFab/API_Specs/blob/80e104aaec1278bd184843809d40344fb129db20/Swagger/PlayFab/Client.swagger.json
[client-sdk]: https://github.com/PlayFab/XPlatCppSdk/tree/06d850bb770752726cfdbe4a0b2be08df5d59d47/code/source/playfab
[authenticate-session]: https://github.com/PlayFab/API_Specs/blob/80e104aaec1278bd184843809d40344fb129db20/Swagger/PlayFab/Server.swagger.json#L283
