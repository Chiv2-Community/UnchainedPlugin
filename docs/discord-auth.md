# Discord authentication foundation

Stacked on the SDK migration in plugin PR #44. Tracks plugin #34–#36;
admission remains disabled.

Ticket verification and signer/verifier compatibility tests live in the backend's
`server-browser-auth-sdk` package (backend PR #21), not this repository.
Native admission does not yet call the verifier; add the published SDK dependency
when that integration is implemented.

The plugin must own one verifier per live registration and serialize ticket
consumption with admission. Trusted key retrieval, cache expiry/rate limits and
connection/player binding still need game integration. Raw travel/connection
options and registration key prefixes are no longer logged. Legacy ban checks
remain fail-open and must not be used as required authentication.

## Local validation

```sh
cargo test --locked -p plugin-backend-integration-tests
cargo test --locked -p plugin-backend-integration-tests --features container-tests
TMPDIR=/tmp/opencode bash scripts/check-windows.sh
```

Container tests use Testcontainers, a local Linux Docker daemon and the published
backend image to check the plugin's ban-check bridge. Linux CI enables them;
Windows builds do not require Docker. These are not native admission evidence.

The Windows type check needs an existing xwin CRT/SDK at `XWIN_ROOT` (defaults to
`~/.cache/cargo-xwin/xwin`), LLVM's clang-cl/llvm-lib/llvm-rc and the installed
Windows Rust target. It downloads nothing and does not run a game. The optional
`WINRES_TOOLKIT_PATH` build setting selects a resource compiler directory; normal
Windows builds retain their existing SDK discovery.

Still gated: browser/prompt/cancellation and join resumption; Steam/Epic/Proton
option-size round trips; connection/player binding through travel; bearer-ticket
transport confidentiality; protected credential storage; OAuth client flow;
host-bot policies and integrated fail-closed admission. No required mode is enabled.
