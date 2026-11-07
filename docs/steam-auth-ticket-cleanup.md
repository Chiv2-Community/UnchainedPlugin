# Steam login probe without temporary tickets

Redpoint's `FLogUserIntoOnlineSubsystemNode` checks whether Steam is logged in
and `GetAuthToken()` returns a nonempty string. Unreal's implementation allocates
a session ticket but discards its cancellation handle. This game-code path does
not cancel the ticket; cleanup is left to the Steam SDK/client lifecycle.

A live trace found 24 outstanding session tickets. Steam's Web API ticket request
rejected them against a limit of 20, returning handle 0. Ordinary session-ticket
requests use a higher limit, so they continued succeeding. The game's separate
invalid-handle callback bug then left EOS login waiting indefinitely.

## Shutdown and concurrent instances

Single-instance tests returned to the original ticket set after normal engine
exit, SIGTERM, and Windows `TerminateProcess`, usually within about 0.3 seconds.
An immediate post-exit snapshot can therefore falsely suggest a leak.

Overlapping unpatched instances behaved differently. Launching A, then B,
allocated one ticket each. After both exited, B's ticket disappeared but A's
remained for at least 60 seconds. This reproduced with either exit order and
when A was force-terminated. In these tests, automatic cleanup covered the
last-launched instance, not necessarily the last-exiting instance.

Follow-up watchpoints located automatic removal inside the Steam process's
32-bit `steamclient.so` on Linux/Proton:

- A process-monitor path checks `ThreadIsProcessIdActive`, then tears down an
  app record and removes tickets matching its game ID and owner token.
- Launching B overwrites the app record's cleanup-owner token. A's existing
  ticket retains A's token, so the later bulk cleanup skips it.
- Calling `SteamAPI_Shutdown` while keeping a diagnostic process alive left
  its ticket outstanding for 3 seconds; process exit triggered removal.

Each game process communicates with Steam through IPC. No second-login
notification is needed to explain the observed owner mismatch. The game
already discards its cancellation handle before any second launch. Internal
owner-token semantics and native Windows behavior remain unverified.

## Workaround

Redpoint first requires `GetLoginStatus() == LoggedIn`. Steam's implementation
at `0x140a494d0` already calls `BLoggedOn()` and returns `LoggedIn` (2) on success.
The patch retains that check and skips the following `GetAuthToken()` call and
length check. No replacement call or marker string is needed.

The existing full helper signature marks the token-block entry with patternsleuth's
`|` offset syntax. A two-byte jump skips to the predicate's success assignment.
The temporary-string cleanup is skipped because its initialized flag remains
zero. This uses the pinned patternsleuth API, not the local matcher branch.

Other token callers are unchanged. Actual EOS authentication still obtains and verifies
its separate Web API ticket; this does not replace authentication or ownership
checks. `BLoggedOn()` reports a live connection to Steam, not game ownership.

Unlike the previous allocate-and-cancel workaround, this allows the predicate
to succeed when Steam is connected but session-ticket allocation would fail.
That allocation never authenticated anything, and its success did not guarantee
that the later Web API ticket request would succeed.

See Steam's documentation for [BLoggedOn](https://partner.steamgames.com/doc/api/ISteamUser#BLoggedOn)
and [GetAuthSessionTicket](https://partner.steamgames.com/doc/api/ISteamUser#GetAuthSessionTicket).

### Why interception is scoped

Static analysis of the tested Steam executable found other callers:

- Steam `AutoLogin` (`0x140a2ddf0`) calls identity slot `+0x130` at
  `0x140a2de42`. Its vtable contains both this AutoLogin implementation and
  `GetAuthToken` (`0x140a44fe0`); this is not Redpoint's scoped helper.
- `0x140ed46d0` obtains an identity interface from a subsystem and copies the
  returned token into an object field at `+0x48` (call `0x140ed4995`).
- An additional candidate, `0x1415051d0`, calls slot `+0x130` at
  `0x1415052d3` and copies the returned string into an array of strings;
  its stored receiver still needs independent identity-type confirmation.
- Frontend function `0x141ac1fa0` obtains the active identity, calls
  `+0x130` at `0x141ac2426`, moves the string into login credentials, and passes
  those credentials to the PlayFab identity's login method.

The latter interface calls can dispatch to different platform implementations;
static analysis does not establish that each executed against Steam in our
tests. They nevertheless consume token contents, unlike Redpoint's length-only
check. A global marker return is not justified. Patching only Redpoint's token
block leaves these other paths on their original implementation.

The patch activates only on Steam. Epic does not require its signature.
The signature assumes the matched helper's instruction layout and control flow;
it covers the preceding status check, token block, and temporary-string cleanup.

This prevents further leaks from this predicate. It does not reclaim existing
tickets, recover an already hung login, or fix the game's invalid-Web-API-handle
callback handling. Steam may still need restarting once to clear prior leaks.

## Validation

The callsite patch was live-tested through Proton on 2026-11-07 without client
RCON or server flags. Process memory contained `EB 26` at `0x140eb1a56`.
After resuming the deliberately suspended diagnostic process, plugin logs showed
frontend initialization, client-mode initialization, travel to the local Heroic
server's Courtyard map, post-load completion, and an in-game chat message.
The client later exited; active ticket snapshots returned 15 before launch and
15 after exit, with no new active handles. The installed DLL was not replaced.

The debugger orchestration failed before login tracing, so this run does not
establish EOS callback results or allocation counts during login. Post-exit
snapshots alone cannot exclude transient allocations. Interactive smoothness
and exit reason were not independently confirmed. Evidence is in the external
investigation directory's `callsite-live-*` files and the diagnostic saved
suffix `AuthDiagnosticCallsiteLive20261107`.

The remaining live results below cover the earlier allocate-and-cancel implementation.

Offline checks found one signature match with the pinned patternsleuth scanner,
at `0x140eb1a56`, and verified the jump target `0x140eb1a7e`. Eighteen emulated
cases covered original/patched code, three login-status values, and empty or
nonempty token strings. The patched path preserved status gating and never
called `GetAuthToken()` or freed its uninitialized temporary. Windows-target
compilation checks and a release DLL build passed. These checks do not replace
live validation. The callsite-patch DLL has SHA-256
`e1addf485bbdcba1c48b7bcf7b68413e27d36e0bc5035c973b052cbea2eeda88`;
the installed DLL was not replaced.

Repository-wide signature validation is tracked in [#28](https://github.com/Chiv2-Community/UnchainedPlugin/issues/28).

Three live tests using the game's Steam API DLL cancelled the same handle
repeatedly, including after allocating a replacement, then shut down normally.
Other outstanding tickets remained intact and each test restored the exact
baseline. This verifies behavior on the tested Steam build, not a general API
guarantee about repeated cancellation.

Two overlapping patched clients kept the exact baseline through both normal
and forced exits. Mixed patched/unpatched runs left the unpatched client's
ticket intact while it was alive; patching a later instance did not reclaim
an earlier unpatched instance's retained ticket. Normal exits returned 0.

For live validation, compare Steam's outstanding-ticket count before launch,
after this probe executes, and after normal and forced exit. Test overlapping
instances in both exit orders and track individual handles, not just counts.
The patched probe should not increase the count or cancel another instance's
ticket. Allow delayed shutdown cleanup before classifying retention.

### Fresh-session login and hosting — 2026-11-07

After restarting Steam, a patched client returned `EOS_Success` from
`EOS_Connect_Login` and completed the frontend login flow using existing saved
setup preferences. Two patched Steam dedicated servers loaded Courtyard
concurrently. A client joined each; reflected network state confirmed a server
connection and player controller, and each host recorded the client connection.
Exiting the first host did not interrupt the client's connection to the second.

Hosting created additional tickets: each host requested two session tickets
and one Web API ticket during startup. Additional allocations alone do not
establish a leak; their ownership and cancellation matter.
Outstanding tickets remained at zero in snapshots after startup, joins,
disconnects, normal shutdown, and forced termination. Inactive ticket records
remained; total record count is not the outstanding-ticket count.

Request tracing covered host startup, not client joins. Join and disconnect
checks used ticket snapshots and reflected network state. The first host and
client exited normally; the second host was force-terminated. All diagnostic
instances exited, the existing server remained running, and the installed
plugin DLL was unchanged; tests injected a separate patched build.

These tests used headless Linux/Proton and local Unchained multiplayer. Native
Windows and official matchmaking remain untested. The origin of the original
24 outstanding tickets remains unknown.

The automated clients used RCON for join commands. The plugin's `is_server()`
checks whether RCON is configured, so these clients exercised plugin server-mode
branches despite lacking Unreal's `-server` flag. The observations above do not
establish behavior for a normal RCON-free client.

### Interactive frontend regression report

An interactive RCON-free patched client hitched and froze after logging probe
cleanup. The process exited before thread stacks could be captured. Timing
alone does not establish causation, but the headless tests do not rule out a
frontend regression. A matching control build with both ticket hooks disabled
was tested next.

The hooks-disabled control and GitHub RC3 release also froze. A later rendered
trace mapped Wine's built-in D3D libraries and `wined3d`, not DXVK: diagnostic
launches called Proton's Wine binary directly, bypassing its normal setup.
This is a launch-path confounder, not proof of the freeze's cause. Interactive
validation must use Proton's normal entry point and verify the graphics backend.

After switching to the proper Proton entry point, mapped D3D library hashes
matched DXVK. The user reported smooth frontend and local multiplayer with the
GitHub RC3 release, then reported the allocate-and-cancel patch also worked.
These interactive results do not yet validate the new `BLoggedOn()` draft.
