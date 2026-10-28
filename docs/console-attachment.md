# Console output options

For game and server operators choosing where plugin output appears. No custom
logging YAML is needed; these options leave configured file logging, filtering,
function prefixes, and namespace formatting unchanged.

## Choose a mode

Add these plugin flags to the game or server process's launch arguments,
preserving its other arguments.

| Goal | Choose |
| --- | --- |
| Forward structured plugin records to a parent process | Supply a valid inherited pipe through `UNCHAINED_CONSOLE_HANDLE`; see the [integration contract](console-forwarding.md). No separate plugin console opens. |
| Capture stdout/stderr instead of structured records | `--attach-stdio`. It also prevents a new plugin console window. |
| Prevent a new plugin console without changing output routing | `--no-spawn-console`. It does not make output visible by itself. |

Without either flag or a valid forwarding handle, the plugin retains its usual
console behavior.
Both flags can be combined, but `--attach-stdio` already prevents console creation.

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
  unchanged and stops reading at EOF.
- Ordinary stdout/stderr writes can block on a slow reader. The default
  structured transport uses a bounded queue to avoid receiver backpressure on
  game threads.

## Capture integration

For structured capture, follow the [Windows handle setup](console-forwarding.md#windows-handle-setup)
and, under Wine/Proton, the [Windows-side capture requirements](console-forwarding.md#wineproton-integration).
For attachment, supply valid stdout/stderr capture handles instead and drain both
streams; merely attaching to a GUI parent does not establish capture. Do not mix
raw game output into a structured JSON stream. Redirected output adds no console
styling, but message-embedded controls still need receiver sanitization.
