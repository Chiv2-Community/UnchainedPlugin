# Attach to standard output

Pass `--attach-std` to the plugin to use inherited stdout/stderr or the parent
console instead of opening a console window. `-attach-std` is also accepted by
the game's argument normalization.

Without the flag, existing console behavior is unchanged. Configured file
appenders, filtering, function prefixes, and namespace formatting are unchanged
in either mode.

## Routing

- The flag is parsed before console initialization and the startup banner.
- Valid inherited stdout/stderr are preserved individually, including pipes,
  files, and mixed redirection. If either is missing, parent-console attachment
  is attempted without replacing a valid inherited destination.
- Attachment failure never falls back to console allocation. With no usable
  output destination, configured file logging remains available.
- Existing console colors are used without changing shared modes/palettes.
  Pipes/files receive plain encoder output; message-embedded controls remain
  the receiver's responsibility.
- Wine's palette fallback must not detach or allocate a console in this mode.
- Explicit `--attach-std` takes precedence over the console-forwarding environment
  variable. It does not use the dedicated structured pipe or emit duplicate records.

The flag does not launch a terminal application or provide interactive command
forwarding. The plugin does not replace stdin; its command reader stops at EOF.
Ordinary standard-stream writes can experience OS pipe backpressure. Use the
bounded [IPC transport](console-forwarding.md) when a nonblocking plugin queue is
required.

## Launcher CLI

The server CLI accepts `--attach-std` directly, or `server.attach-std: true` in YAML.
It captures both streams into the existing TUI/plain-output handler rather than
letting game output overwrite the dashboard. `L` selects the console pane;
`End` follows output. Captured lines/history are bounded and controls sanitized.

On Windows, suspended startup uses private stdout/stderr pipes and an explicit
handle-inheritance whitelist. The child receives EOF on stdin, not the TUI's
keyboard input. On Linux, Proton's `runinprefix` verb preserves the helper's
inherited streams; the helper captures the suspended Windows game's output and
relays it to the native CLI. Steam must still be running for the Steam game.

The CLI flag is a server option. Direct plugin launches can use the plugin flag
independently. GUI launchers must supply valid capture handles; merely attaching
to a GUI parent does not establish output capture.
