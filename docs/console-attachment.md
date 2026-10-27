# Console output options

For game and server operators choosing where plugin output appears. No custom
logging YAML is needed; these options leave configured file logging, filtering,
function prefixes, and namespace formatting unchanged.

## Choose a mode

With a matching UnchainedLauncher CLI/plugin build, add options to your existing
`server` command, keeping its other arguments. These CLI options apply to
`server`, not `client`. For a direct launch with the plugin loaded, add the flags
to the game process's launch arguments instead.

| Goal | Choose |
| --- | --- |
| See plugin logs in the server CLI | Neither flag. Structured capture is automatic; no separate plugin console opens. |
| Capture stdout/stderr instead of structured records | `--attach-stdio`. It also prevents a new plugin console window. |
| Prevent a new plugin console without changing output routing | `--no-spawn-console`. It does not make output visible by itself. |

Without either flag, direct plugin launches retain their usual console behavior.
Both flags can be combined, but `--attach-stdio` already prevents console creation.
Older plugins without capture support may still open their own console.

### View server CLI output

Press `L` to switch between the dashboard and console pane, Page Up/Page Down to
scroll, and `End` to follow new output. No-TUI mode prints received lines.
Captured history is bounded and terminal controls are sanitized.

By default, only structured plugin records are displayed; game stdout/stderr are
drained and discarded. With `--attach-stdio`, stdout/stderr are displayed instead.

### Optional launcher YAML

Instead of the command-line flag, add the setting under your existing `server:`
section in the **launcher configuration**, not the log4rs logging file:

```yaml
server:
  attach-stdio: true
```

Use `no-spawn-console: true` instead to suppress console creation without selecting
attachment. Both keys may be enabled together. Keep your other settings and do not
add a second `server:` section.

## What the flags change

`--attach-stdio` preserves valid inherited stdout/stderr, including files, pipes,
and mixed redirection. If a destination is missing, it tries the parent console
without replacing a valid inherited destination. It overrides structured
forwarding, selects output before logging and the banner, and never falls back
to creating a console. Shared console modes/palettes are not changed.

`--no-spawn-console` prevents plugin console allocation, including Wine's palette
fallback. It does not attach to a parent console, redirect streams, or disable
structured forwarding. Existing output handles remain usable; a direct launch
without usable handles or forwarding may have no visible console output.

Game argument normalization also accepts `-attach-stdio` and `-no-spawn-console`.
The old name `--attach-std` is not an alias.

## Limits and troubleshooting

- If attachment has no usable destination, no replacement window opens. Check
  the [configured log file](logging.md#log-location).
- These flags do not provide command forwarding. The plugin leaves stdin
  unchanged and stops reading at EOF. The server CLI supplies EOF to the game;
  its keyboard controls do not become game stdin.
- Ordinary stdout/stderr writes can block on a slow reader. The default
  structured transport uses a bounded queue to avoid receiver backpressure on
  game threads.
- Steam must remain running when hosting the Steam game through Proton.

## Launcher authors

For structured capture, follow the [Windows handle setup](console-forwarding.md#windows-handle-setup)
and, on Linux, the [Proton helper recipe](console-forwarding.md#alternate-linux-launchers).
For attachment, supply valid stdout/stderr capture handles instead and drain both
streams; merely attaching to a GUI parent does not establish capture. Do not mix
raw game output into a structured JSON stream. Redirected output adds no console
styling, but message-embedded controls still need receiver sanitization.
