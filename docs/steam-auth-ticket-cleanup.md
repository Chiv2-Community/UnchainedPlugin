# Steam auth token failures

Redpoint's login check allocates and discards a Steam session ticket. Tickets
retained across launches can cause EOS login to hang on the frontend.

## Original Behavior

1. `GetLoginStatus()` checks Steam login.
2. `GetAuthToken()` allocates a session ticket; Redpoint checks its length.
3. Unreal discards the cancellation handle; Redpoint discards the token.

## The Problem

Retained tickets can exhaust the separate Web API ticket quota (20 in the tested
Steam build). The game's handling of the failed request then hangs EOS login.

## Workaround

Remove this unused token allocation and length check. Keep Steam login and
downstream EOS authentication unchanged.

## Reasoning

* The discarded ticket authenticates nothing in this check.
* Steam login was already checked; EOS obtains its credentials separately.
* The only removed requirement is that this unused allocation succeeds.

See Steam's documentation for [BLoggedOn](https://partner.steamgames.com/doc/api/ISteamUser#BLoggedOn)
and [GetAuthSessionTicket](https://partner.steamgames.com/doc/api/ISteamUser#GetAuthSessionTicket).
