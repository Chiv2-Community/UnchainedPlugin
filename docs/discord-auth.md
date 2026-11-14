# Discord authentication foundation

Stacked on the SDK migration in plugin PR #44. Tracks plugin #34–#36;
admission remains disabled. See [backend-sdk.md](backend-sdk.md) for the migration.

## Generated backend integration

The backend owns generated Rust model/client crates, templates and drift checks.
The plugin consumes exact backend Git revisions without its own OAS snapshot or
generator. `unchained-auth` depends only on the model crate. Backend PR #21
declares 64-bit claim timestamps and regenerates both language contracts.

Registration, updates, heartbeat and deletion use `server-browser-client-sdk`
directly. Ban checks use generated bindings directly. `src/backend.rs` adds the
plugin user-agent to the SDK transport; deadlines, redirect suppression,
source-IP binding and safe debug output remain SDK-owned. Legacy ban checks
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
cargo test --locked -p unchained-auth -p plugin-portable-tests
cargo test --locked -p plugin-portable-tests --features container-tests
TMPDIR=/tmp/opencode bash scripts/check-windows.sh
```

The integration tests use Testcontainers and a local Linux Docker daemon. They
build the authoritative backend Dockerfile from the exact SDK Git revision,
create private signing keys inside disposable containers, exercise registration
lifecycle operations, and verify a backend-signed ticket for a live registration
with replay and wrong-audience rejection. Keys never leave the container; tickets
are checked for log leakage. No host Python environment, backend checkout,
hosted service or Discord API is needed. This is not OAuth or native admission
evidence. Linux CI enables container tests; Windows builds do not require Docker.

The Windows type check needs an existing xwin CRT/SDK at `XWIN_ROOT` (defaults to
`~/.cache/cargo-xwin/xwin`), LLVM's clang-cl/llvm-lib/llvm-rc and the installed
Windows Rust target. It downloads nothing and does not run a game. The optional
`WINRES_TOOLKIT_PATH` build setting selects a resource compiler directory; normal
Windows builds retain their existing SDK discovery.

Still gated: browser/prompt/cancellation and join resumption; Steam/Epic/Proton
option-size round trips; connection/player binding through travel; bearer-ticket
transport confidentiality; protected credential storage; OAuth client flow;
host-bot policies and integrated fail-closed admission. No required mode is enabled.
