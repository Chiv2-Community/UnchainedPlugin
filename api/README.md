# Backend OAS snapshot

`server-browser.yaml` is copied from C2ServerBrowserBackend commit `14c8046`
([backend PR #21](https://github.com/Chiv2-Community/C2ServerBrowserBackend/pull/21)).
That change declares 64-bit Unix timestamps; other operations match backend
`v1.3.0-alpha`. The backend remains authoritative. Update the backend schema first,
then refresh this snapshot and generated Rust SDK with
`scripts/generate-server-browser.py --backend-root <backend checkout>`.

OAuth/start/callback/refresh/logout/ticket endpoints are not defined yet. Add them
to the authoritative backend OAS before generating their plugin clients.
