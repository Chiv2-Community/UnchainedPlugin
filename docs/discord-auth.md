# Discord authentication foundation

Stacked on the SDK migration in plugin PR #42. Tracks plugin #34–#36;
admission remains disabled. See [backend-sdk.md](backend-sdk.md) for the migration.

## Generated backend integration

`crates/server-browser-api` owns generated Rust routes/models and its generation
templates. `api/server-browser.yaml` is a pinned copy of the backend's authoritative
OAS, not an independently edited contract. OpenAPI Generator is pinned to 7.17.0.
Java 11+ and uv are needed for generation, not runtime.

```sh
uv run --script scripts/generate-server-browser.py --backend-root ../C2ServerBrowserBackend-auth-sdk
uv run --script scripts/generate-server-browser.py --check
```

The module's `anyOf` template preserves ordinary and diagnostic listings. Reference
variants are tried most-specific-first, including inherited required fields.
Generation drift checks compare every generated Rust source and the manifest.
The snapshot preserves 64-bit claim timestamps from the backend OAS.

Registration, updates, heartbeat, deletion and ban checks use generated operations
and models through `unchained-backend`. The adapter centralizes deadlines,
redirect suppression, source-IP binding and safe debug output. Legacy ban checks
remain fail-open; that behavior must not be used for required authentication.
Existing engine-owned HTTP redirects remain engine hooks, not replacement HTTP
clients. The generated SDK includes their compatibility endpoints as well.

Define OAuth/session/ticket HTTP operations in the backend OAS before adding any
client calls. Do not introduce handwritten OAuth URLs or wire DTOs.

## Ticket verification

`unchained-auth` uses generated `JoinTicketClaims`/JWKS models. It verifies fixed
RS256/type/issuer/registration audience, canonical Discord IDs/UUIDs, required
claims, original authentication time, maximum 60-second lifetime and 30-second skew.
Header-supplied key URLs/embedded keys are rejected. Unknown keys cause no fetch.
Token/JWKS sizes, key counts and replay history are bounded. Full replay capacity
rejects admission without evicting live entries. Key replacement preserves replay
history and failed replacement leaves the old keys intact.

One verifier belongs to one live registration. Serialize verification/consumption
with admission. Trusted key retrieval, cache expiry/rate limits and connection
binding still need game integration. The crate is intentionally not linked into
the native admission hooks yet. Raw travel/connection options and registration
key prefixes are no longer logged.

## Local validation

```sh
cargo test --locked -p unchained-auth -p unchained-backend
uv run --project ../C2ServerBrowserBackend-auth-sdk python scripts/check-auth-backend.py
TMPDIR=/tmp/opencode bash scripts/check-windows.sh
```

The interop script hosts Uvicorn on this machine's loopback interface, uses a
temporary private signing keyring, exercises generated browser operations, and
verifies a real backend-signed ticket through Rust with replay rejection. It uses
no hosted backend or Discord API. This is not OAuth or native admission evidence.

The Windows type check needs an existing xwin CRT/SDK at `XWIN_ROOT` (defaults to
`~/.cache/cargo-xwin/xwin`), LLVM's clang-cl/llvm-lib/llvm-rc and the installed
Windows Rust target. It downloads nothing and does not run a game. The optional
`WINRES_TOOLKIT_PATH` build setting selects a resource compiler directory; normal
Windows builds retain their existing SDK discovery.

Still gated: browser/prompt/cancellation and join resumption; Steam/Epic/Proton
option-size round trips; connection/player binding through travel; bearer-ticket
transport confidentiality; protected credential storage; OAuth client flow;
host-bot policies and integrated fail-closed admission. No required mode is enabled.
