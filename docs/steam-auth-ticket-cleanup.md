# Steam login probe ticket cleanup

Redpoint's `FLogUserIntoOnlineSubsystemNode` checks whether Steam is logged in
and `GetAuthToken()` returns a nonempty string. Unreal's implementation allocates
a session ticket but discards its cancellation handle. Forced game termination
can leave that ticket counted in Steam's per-app client state.

A live trace found 24 outstanding session tickets. Steam's Web API ticket request
rejected them against a limit of 20, returning handle 0. Ordinary session-ticket
requests use a higher limit, so they continued succeeding. The game's separate
invalid-handle callback bug then left EOS login waiting indefinitely.

## Workaround

1. Hooks Redpoint's login helper and Steam's `GetAuthToken()`.
2. Records the helper's identity and local user for its next token check.
3. Cancels that check's temporary ticket immediately after requesting it.

The check still requires a valid, nonempty ticket. Other token callers are unchanged.

Both hooks activate only on Steam. The Redpoint wrapper checks that the token
hook is enabled before opening a scope. Epic does not require these signatures.

The signatures assume this game's ABI, object offsets, and SteamUser023 vtable layout.

This prevents further leaks from this predicate. It does not reclaim existing
tickets, recover an already hung login, or fix the game's invalid-Web-API-handle
callback handling. Steam may still need restarting once to clear prior leaks.

## Validation

Repository-wide signature validation is tracked in [#28](https://github.com/Chiv2-Community/UnchainedPlugin/issues/28).

For live validation, compare Steam's outstanding-ticket count before launch,
after this probe executes, and after forced game termination. The probe should
not increase the count; normal authentication still needs separate testing.
