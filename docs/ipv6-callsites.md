# IPv6 browser patches

The backend continues to send IPv6 hosts as `[address]` in `ServerIPV4Address`.
No separate IPv6 JSON field or backend change is required. The game's generated
GetCurrentGames model does not parse an IPv6 field.

Client preinit redirects four calls, once:

- OnGetCurrentGames's colon search preserves an IPv6 host before appending
  `PINGPORT_i`. Its separate game connect string retains the game port.
- Both PingSomeServers searches use the final separator for IPv6 endpoints,
  keeping full host keys for cached and in-flight pings.
- UDPEchoImpl's splitter returns an unbracketed IPv6 host and its port.

Other inputs delegate to the original string functions. Neither FString entry
point nor OnGetCurrentGames, PingSomeServers, or UDPEcho is detoured. Server
processes skip the patches. Bare IPv6 ping endpoints retain the previous
implementation's final-colon convention; the backend should send bracketed hosts
so the game connect URL is also valid.

Signatures resolve the call instructions. Installation validates all four E8
instructions and their original targets before changing any code. A nearby RX
relay page supports replacement functions outside rel32 range without changing
argument registers or stack layout. All call pages are made writable first,
then patched and restored in reverse order. These writes are only safe during
preinit, before game threads execute the browser paths. Validation/allocation/
protection failures leave the call instructions unchanged; failure to restore
page protections aborts startup rather than continuing in an unknown state.

## Validation

Portable logic and instruction-encoding tests:

```sh
rustc --edition 2021 --test tests/ipv6_callsites.rs -o /tmp/opencode/ipv6-tests
/tmp/opencode/ipv6-tests
```

Windows tests also execute synthetic native call sites to verify five-argument
ABI forwarding, whole-batch validation, unchanged unrelated calls, and restored
page protections:

```sh
cargo test --test ipv6_callsites
```

Read-only checks against executable files verify unique signatures and callees:

```sh
python3 tests/check_ipv6_signatures.py '/path/to/Chivalry2-Win64-Shipping.exe'
```

The signatures were checked against the installed Steam executable and the older
PDB-bearing executable. Other builds must pass the same checks; no absolute VAs
are used by the patches. Live IPv4/IPv6 browser pings and joins still require a
game-client test.
