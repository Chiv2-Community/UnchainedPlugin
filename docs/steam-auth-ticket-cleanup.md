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

Both hooks are inert unless the supported Steam code signatures match. The
Redpoint wrapper also checks that the token hook is enabled before opening a
scope. Epic and unknown builds keep their original behavior.

Supported executable SHA-256:
`d82b5eaa122aaee15366499bbce34e325741fecd06d6b7b5490fc650b5afb48b`.
Only this build (CRC32C `04d195bc`) requires the new signatures during scanning.
The signature/RVA guards cover Redpoint helper `0xEB1A10` and Steam
`GetAuthToken` `0xA44FE0`. They encode this build's ABI, object offsets, and
SteamUser023 vtable layout. Recheck these assumptions before adding builds.

This prevents further leaks from this predicate. It does not reclaim existing
tickets, recover an already hung login, or fix the game's invalid-Web-API-handle
callback handling. Steam may still need restarting once to clear prior leaks.

## Tests

Run the dependency-free lifecycle/scope tests on any host:

```sh
rustc --edition=2021 -O --test tests/steam_auth_policy.rs -o steam-auth-tests
./steam-auth-tests
```

Check signature uniqueness against a local supported executable:

```sh
STEAM_AUTH_TEST_EXE='/path/to/Chivalry2-Win64-Shipping.exe' \
  ./steam-auth-tests --ignored
```

On Windows, the tests also run through `cargo test --test steam_auth_policy`.
For live validation, compare Steam's outstanding-ticket count before launch,
after this probe executes, and after forced game termination. The probe should
not increase the count; normal authentication still needs separate testing.
