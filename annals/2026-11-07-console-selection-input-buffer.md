# Console selection with redirected input

- Date: 2026-11-07 local (2026-11-08 UTC).
- Status: proposed fix, implemented but uncommitted; live selection unverified.
- Scope: plugin-owned Windows console input mode.
- Repository: [UnchainedPlugin](https://github.com/Chiv2-Community/UnchainedPlugin.git).
- Branch: `log-standardization-finalization`; recording HEAD `62bb1a7d10b9c25cfa08ba9978ce5e978e06750c`; no exact tag, nearest tag `v1.0.0-RC3`. Baseline implementation `3cfc8c1db58323970a39207401949434c06c50f6`, [PR #30](https://github.com/Chiv2-Community/UnchainedPlugin/pull/30). Two source files have uncommitted changes; the fix is not in that PR commit.
- Source: user and build agent, session `ses_e5658faf6ffewA56PWUxX9QcPT` through response `msg_1b9773c1d0010uMenlpHKiMLYm`. Earlier history is compacted.

## Decision and evidence

The user reported being unable to select text in the running preview, despite the prior [queued-console design](2026-11-07-console-logging-standardization.md). Its earlier test had explicitly supplied a console input handle. The assistant identified redirected or invalid standard input as a plausible failure path, not a confirmed root cause: a probe of the original preview failed after that process had closed.

The uncommitted change opens `CONIN$` directly for a plugin-owned console and sets QuickEdit and extended flags while disabling mouse input, without replacing inherited stdin. The alternative, relying on `STD_INPUT_HANDLE`, can silently skip configuration when that handle is redirected. Shared consoles retain the previous behavior. This aims to preserve redirected input and make selection available without changing the queued-output decision.

The local diff in `src/logging/console/windows.rs` and `src/logging/log4rs/console.rs` confirms the implementation and a redirected-`NUL` regression fixture. The available Proton test log reports that fixture passed. A rebuilt Steam preview initialized; its probe report recorded mode `0x1e7`, consistent with the intended flags. The probe command timed out even though its report was written. This confirms input mode, not that dragging actually selects text or that gameplay remains responsive during selection. Native Windows and end-to-end file logging during selection remain unverified.

## Consequences and follow-up

The change is a targeted hypothesis and may not explain the user's original failure. Confirm actual drag selection and gameplay responsiveness in the preview, test native Windows and redirected/shared console cases, and review the complete diff before committing or updating PR #30. Reconsider direct buffer access if it fails on supported console hosts or affects inherited handles. The user's subsequent confirmation and the original process's input mode are unavailable.
