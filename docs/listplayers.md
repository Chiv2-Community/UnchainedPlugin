# ListPlayers output

`ListPlayers` copies a player table to the clipboard and prints the same table
to the invoking local player's in-game console. The plugin preserves the
native server-name/address line and replaces the table with seven columns:
name, PlayFab ID, base unique ID, score, kills, deaths and ping.

- Columns are left-aligned, sized from the widest header or value in that
  invocation by `tabled`. Unicode display widths are used; view the result in a
  monospace font.
- `UniqueNetId` replaces the misleading native `EOSPlayerId` heading. The base
  wrapper is not necessarily an EOS product-user ID.
- Missing IDs and empty cells display `NULL`. Duplicate names and the server's
  local bootstrap player remain visible. Bots remain excluded, matching native behavior.
- Ping is the native compressed ping byte multiplied by four, in milliseconds.
- Control characters in cells become spaces so names cannot inject extra rows.
- No identity, authentication state or player statistics are modified.
- Console output uses native `ClientMessage`, one line at a time, without sending
  player IDs to chat, chat-event subscribers or other players. Headless/server
  invocations without a local player keep clipboard output only.

## Implementation

The native row format lacks its third string conversion. It interprets the base-ID
string pointer as an integer, shifts subsequent statistics, and omits actual ping.
See [issue #46](https://github.com/Chiv2-Community/UnchainedPlugin/issues/46).

The plugin replaces `ListPlayers` directly. It builds the server header and
player table once, prints them to the invoking local console, then calls the
native clipboard function. There is no clipboard hook or thread-local state,
and the broken original command never runs.

Only `ListPlayers` is intercepted. Non-virtual clipboard, `FText::ToString` and
`APlayerState::GetPlayerName` functions use registered callables without enabling
interception. `FText::ToString` must rebuild text before reading its display string;
calling its data vtable directly would skip that work.
The server name and player list use the existing global world/game-state access,
with TBL player-state filtering. IDs use a typed partial vtable; player names call
native `APlayerState::GetPlayerName` rather than duplicating its custom-name logic.
Vtables stay private; their owning structs expose `#[inline(always)]` forwarding methods.
The server endpoint uses `UIpNetDriver::LocalAddr` on hosts or
`UIpConnection::RemoteAddr` on clients, calling native
[`FInternetAddr::ToString(bool bAppendPort) const`](https://github.com/folgerwang/UnrealEngine/blob/release/Engine/Source/Runtime/Sockets/Public/IPAddress.h)
through its reconstructed vtable with the port included. Endpoint selection stays
in the command, not in a custom net-driver method. No variadic
formatter or unrelated clipboard behavior is changed.

Function signatures were checked on the installed Steam and EGS builds; addresses
are not hard-coded. Different executable layouts require revalidation. The broken
original command is never used as a fallback.

## Tests

`cargo test --lib resolvers::listplayers` on Windows covers cell presence,
including empty lists, missing values, duplicate names, Unicode, percent signs
and control characters. `cargo test --lib unique_net_id_tests` covers missing and
invalid ID handling. Output tests use no exact-string snapshots;
width calculation and padding are delegated to `tabled`.

Native offsets and vtable signatures require validation against the actual game
binary and runtime; unit tests cannot establish compatibility after game updates.

In-game console and clipboard behavior still need runtime validation. Use a
coordinated or fully isolated game environment; separate ports and save suffixes
do not isolate a shared Wine prefix or signature cache.
