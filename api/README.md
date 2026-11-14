# Backend OAS snapshot

`server-browser.yaml` is copied from C2ServerBrowserBackend `v1.3.0-alpha`
(commit `4849107`). The backend remains authoritative. Update the backend schema first,
then refresh this snapshot and generated Rust SDK with
`scripts/generate-server-browser.py --backend-root <backend checkout>`.

OAuth/start/callback/refresh/logout/ticket endpoints are not defined yet. Add them
to the authoritative backend OAS before generating their plugin clients.
