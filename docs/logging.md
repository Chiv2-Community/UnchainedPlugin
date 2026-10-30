# Logging

Use a custom log4rs YAML file to control which plugin messages appear in the
console and log files. No configuration file is required for the defaults.

For machine-readable output, see [structured logging](#structured-logging).
Contributors: see [emitting diagnostics](#contributor-guide-emitting-diagnostics).

## Defaults

The plugin uses its [embedded configuration](../src/logging/log4rs/log4rs.default.yaml)
unless you pass `--log4rs-yaml`.

Startup logs parsed CLI settings at DEBUG. `CLIArgs::Debug` masks passwords,
Discord tokens, backend URLs, and all INI/passthrough contents (counts only).
Raw command lines are not logged. Other structured fields remain visible;
do not put credentials in profile names, paths, MOTDs, or other public settings.

### Log location

The default main file is:

```text
%LOCALAPPDATA%\Chivalry 2\Saved_<saveddirsuffix>\Logs\unchained.log
```

For example, `--saveddirsuffix Steam` gives:

```text
%LOCALAPPDATA%\Chivalry 2\Saved_Steam\Logs\unchained.log
```

Use the Windows account running the game or server when locating `%LOCALAPPDATA%`.
The game's launch arguments may specify a different saved-directory suffix.

### Console colors

| Level | Header | Message body |
| --- | --- | --- |
| ERROR | Pastel red | Light gray (`#eeeeee`) |
| WARN | Pastel yellow | Light gray (`#eeeeee`) |
| INFO | Muted pastel blue | Light gray (`#eeeeee`) |
| DEBUG | Dark gray (`#808080`) | Dark gray |
| TRACE | Darker gray (`#5c5c5c`) | Darker gray |

Message color codes are preserved and can override the body color. Pastel shades
use RGB terminal colors when supported. Wine's graphical console and legacy
Windows consoles instead receive a matching 16-color palette when the plugin
owns the console. Wine needs a dedicated console created with temporary,
application-specific registry defaults; previous values are restored immediately
after creation, and global console defaults are not changed. Other programs sharing
that console see the same palette, including altered shades for embedded colors.
Shared consoles, additional palettes, and rejected palette changes use the
nearest available console colors without changing the palette.
The default file appender and redirected output receive no added console colors;
color codes already present in a message are not stripped.

The startup banner is an ordinary `INFO` log record. It follows configured
appenders, encoders, and filters instead of writing directly to stdout.
Its `title` palette entry defaults to bright yellow (`#ffff00`), and `subtitle`
defaults to bright red (`#ff0000`). Override those entries to change banner colors.

## Use a custom configuration

1. Download or copy the [default YAML](../src/logging/log4rs/log4rs.default.yaml) to a local
   file, such as `C:\Unchained\logging.yaml`.
2. Edit that copy. A custom file **replaces the entire configuration**; it does
   not merge with the defaults. Keep the dependency filters unless you want more
   library output.
3. Add this option to the Chivalry 2 game or server process's launch arguments,
   preserving the other arguments:

   ```text
   --log4rs-yaml "C:\Unchained\logging.yaml"
   ```

4. Restart the game or server after changing the file. Configuration is loaded
   at startup, not automatically reloaded.

Use an absolute path. Relative paths resolve from the game process's working
directory, which may differ from the folder containing your startup script.

Keep the quoted `"__UNCHAINED_LOG_PATH__"` and `"__KISMET_LOG_PATH__"` values to
use the plugin's default file locations. The plugin replaces these tokens at
startup. To choose a different file, use a writable path; forward slashes avoid
YAML backslash escaping:

```yaml
path: "C:/Unchained/Logs/unchained.log"
```

Do not substitute `%LOCALAPPDATA%` into the YAML and expect it to expand like the
plugin's path tokens.

## Structured logging

In your [custom configuration](#use-a-custom-configuration), select the standard
[log4rs JSON encoder](https://docs.rs/log4rs/latest/log4rs/encode/json/index.html)
on each console or file appender that should emit JSON:

```yaml
encoder:
  kind: json
```

Each configured sink emits one JSON payload per line. Standard log4rs fields
include `time`, `level`, `message`, source locations, and thread metadata, all at
the root. Plugin macros add the enclosing function name as
`attributes.enclosing_function`.

`--attach-stdio` reuses inherited stdout/stderr or attaches to the parent console;
it does not create redirection. The calling program supplies pipes or files when
capture is needed. `--no-spawn-console` independently suppresses console allocation.
**Some initialization output is not in the configured format**, including
diagnostics written before log4rs initializes. Stdio may also contain game and
dependency output, so consumers may encounter non-JSON lines alongside JSON logs.

## Format log lines

The `palette` encoder combines log4rs patterns with a named color table:

```yaml
encoder:
  kind: palette
  pattern: "{palette({l})}[{d(%H:%M:%S)} {l:5}]{palette({l}_body)} [{sm:<-24.24}] {m}{n}"
  palette:
    error: "#f28b82"
    warn: "#f2cc78"
    info: "#8cb4e1"
    debug: "#808080"
    trace: "#5c5c5c"
    error_body: "#eeeeee"
    warn_body: "#eeeeee"
    info_body: "#eeeeee"
    debug_body: "#808080"
    trace_body: "#5c5c5c"
    title: "#ffff00"
    subtitle: "#ff0000"
```

These are the defaults. Add any named colors using quoted `#RRGGBB` values;
`null` uses the console's normal text color. `{palette(name)}` switches colors
until the next directive or end of the record; `{palette()}` restores normal text.
Names are ASCII case-insensitive. `{l}` or `{level}` in a name selects a color by
severity, so `{palette({l}_body)}` selects `error_body`, `warn_body`, etc.
References are resolved at startup; unknown names reject the configuration.
Directives must be outside other formatters. An explicit `palette` replaces the
default table; omit it to use the defaults. Legacy consoles approximate colors
beyond their available slots.
Console coloring respects `NO_COLOR`, `CLICOLOR`, and `CLICOLOR_FORCE` through
log4rs. File output has no added colors. Restart after editing.

The `{sm}` field abbreviates parent module segments using
case-preserving word initials: `foo_bar` becomes `fb`, `UnchainedPlugin` becomes
`UP`, and `HTTPServer` becomes `HS`. The final segment is not abbreviated.
The resulting label keeps its last 32 Unicode characters, dropping any excess on the left:
`UnchainedPlugin::resolvers::messages` becomes `UP::r::messages`.
The default console pattern `{sm:<-24.24}` uses exactly 24 Unicode characters
inside the brackets: shorter labels are padded on the right; longer labels retain
their last 24 characters. Custom `{sm}` keeps the 32-character cap and adds no
padding unless a width is specified.
File appenders use `{M}` for the full source module. Both defaults use `{l:5}`
for the level; `{t}` shows the filtering target instead.

Console and file patterns support the same fields, including `{sm}`, `{fn}`,
`{M}`, `{t}`, source locations, and user key/value fields. This includes `file`
and `rolling_file` appenders, with explicit or omitted `kind: pattern`.
Nested formatters, escaping, and width specifications such as `{sm:>16}` work
in both. To omit or replace the console's short module, edit `pattern`.
`kind: pattern` uses a single `pattern` field and does not apply the console's
pastel severity styling. `{sm}` is our extension, not a built-in log4rs field;
`{M}` and `{t}` retain their standard meanings.

Console and file defaults omit function names. Add `{fn}` to display the enclosing
named function's leaf. Methods show only their name,
without the type or module. Closures and async blocks use their enclosing named
function, not `{{closure}}`.
Records from dependencies without function metadata show `???`.
Like `{sm}`, `{fn}` supports nesting, escaping (`{{fn}}`), and width (`{fn:>20}`)
through the built-in log4rs key/value formatter.

For example, replace a file appender's encoder block to include the short module
and opt-in function field without severity colors:

```yaml
encoder:
  kind: pattern
  pattern: "[{d(%H:%M:%S)} {l:5}] [{sm}::{fn}] {m}{n}"
```

See the [log4rs pattern reference](https://docs.rs/log4rs/latest/log4rs/encode/pattern/index.html)
for available fields and formatting options.

## Set log levels

Levels are `off`, `error`, `warn`, `info`, `debug`, and `trace`. A level includes
messages at that severity and higher: `warn` includes WARN and ERROR; `trace`
includes every severity. `off` disables the selected namespace.

Add namespace entries under the existing `loggers:` mapping. For example:

```yaml
loggers:
  # Keep the existing dependency entries here too.
  "UnchainedPlugin::features::server_registration":
    level: info
  "UnchainedPlugin::modules::vote":
    level: warn
  "UnchainedPlugin::resolvers::messages::client_message::game_chat":
    level: off
```

These are additions to your copied file, not a complete configuration. Do not
create a second `loggers:` mapping.

A namespace setting applies to its descendants unless a more specific setting
overrides it. For example, `UnchainedPlugin::modules::discord` controls all Discord
modules, while `UnchainedPlugin::modules::discord::event_log` controls just the
event log. Names are case-sensitive; use full target namespaces, not the shortened
console names. Both console and file output show source modules by default;
explicit targets can differ. Add `{t}` to a file pattern to inspect those targets.

Changing `root.level` changes the fallback for every namespace without a more
specific override, including dependencies. To change only plugin output, set
`UnchainedPlugin` under `loggers` instead.

### Enable TRACE output

To trace combat events without enabling trace everywhere, add this entry under
`loggers`:

```yaml
"UnchainedPlugin::resolvers::gameplay":
  level: trace
```

For console output, also change the existing console threshold:

```yaml
appenders:
  console:
    kind: console
    filters:
      - kind: threshold
        level: trace
    encoder:
      kind: palette
```

Keep the other appenders and root settings in your copied file. Both changes are
needed to see these TRACE records on the console. The default file appender has
no threshold filter, so enabled TRACE records already reach `unchained.log`.

Enabling a level does not itself generate messages: reproduce the action you
want to investigate. Logging settings cannot enable a feature missing from your
plugin build.

## Namespace reference

All namespaces below default to **`debug`**, inherited from the root. This lists
the plugin's current logging targets and their parent groups. Modules with no
log messages are omitted. Some targets emit only when their feature is enabled
or the relevant action occurs.

### Parent groups

| Namespace | Controls |
| --- | --- |
| `UnchainedPlugin` | All plugin logs, including startup, initialization, and patch status. |
| `UnchainedPlugin::events` | Event and broadcast diagnostics. |
| `UnchainedPlugin::features` | Discord, mod management, remote console, and server registration. |
| `UnchainedPlugin::game` | Game-state access and engine helpers. |
| `UnchainedPlugin::modules` | Commands, chat, votes, and Discord modules. |
| `UnchainedPlugin::modules::discord` | Discord admin alerts, dashboard, and event log. |
| `UnchainedPlugin::modules::votes` | Individual vote actions. |
| `UnchainedPlugin::resolvers` | Game integration, asset loading, chat, combat, and map transitions. |
| `UnchainedPlugin::tools` | Logging setup, startup arguments, and low-level initialization. |

### Individual targets

| Namespace | What it reports |
| --- | --- |
| `UnchainedPlugin::commands` | Invalid command arguments and command parsing failures. |
| `UnchainedPlugin::events::broadcast_message::console` | Broadcast messages sent to the console. |
| `UnchainedPlugin::events::models` | Unknown chat types and fallback handling. |
| `UnchainedPlugin::features::discord` | Discord connection, channel configuration, and message delivery failures. |
| `UnchainedPlugin::features::mod_management` | Mod discovery, spawning, enabled mods, and saved mod settings. |
| `UnchainedPlugin::features::rcon` | Remote-console listener, received commands, and connection failures. |
| `UnchainedPlugin::features::server_registration` | Server discovery, listing registration, authentication, and heartbeats. |
| `UnchainedPlugin::game::chivalry2` | Game-state access failures, player counts, and in-game message delivery. |
| `UnchainedPlugin::game::engine` | Asset-name and engine-interface lookup failures. |
| `UnchainedPlugin::modules::chat` | Chat messages handled by the chat module. |
| `UnchainedPlugin::modules::command` | Executed commands and generated command help. |
| `UnchainedPlugin::modules::discord::admin_alert` | Admin-alert configuration problems. |
| `UnchainedPlugin::modules::discord::dashboard` | Discord dashboard configuration problems. |
| `UnchainedPlugin::modules::discord::event_log` | Discord event-log queue overload and dropped events. |
| `UnchainedPlugin::modules::game_info` | Requested game/player details and unavailable game state. |
| `UnchainedPlugin::modules::mod_dump` | Saving the mod registry to a file. |
| `UnchainedPlugin::modules::mod_list` | Mod-list refreshes and scans. |
| `UnchainedPlugin::modules::vote` | Vote cancellation. |
| `UnchainedPlugin::modules::votes::votekick` | Player kicks performed by a vote. |
| `UnchainedPlugin::resolvers::admin_control` | In-game administration commands being executed. |
| `UnchainedPlugin::resolvers::asset_loading` | Save-game loading and saving. |
| `UnchainedPlugin::resolvers::asset_registry` | Asset discovery, class lookups, and object loading. |
| `UnchainedPlugin::resolvers::backend_hooks` | Backend ban checks, player connections, and departures. |
| `UnchainedPlugin::resolvers::config_overrides` | Per-map/profile INI override selection; requires a build with `config_overrides_verbose_hooks`. |
| `UnchainedPlugin::resolvers::desync` | Blocked game events and dedicated-server compatibility checks. |
| `UnchainedPlugin::resolvers::etc_hooks` | Map summaries, travel, and assorted game calls; also override-load warnings in verbose builds. |
| `UnchainedPlugin::resolvers::gameplay` | Combat actor validation and TRACE details for damage and kills. |
| `UnchainedPlugin::resolvers::getpost_requests` | Backend request handling and request-function discovery. |
| `UnchainedPlugin::resolvers::messages` | Game console output and chat-broadcast calls; also controls the chat targets below. |
| `UnchainedPlugin::resolvers::messages::client_message` | Parent for received player chat and system messages. |
| `UnchainedPlugin::resolvers::messages::client_message::game_chat` | Received player chat messages. |
| `UnchainedPlugin::resolvers::messages::client_message::system_chat` | Received server/system messages, MOTD, and chat parsing warnings. |
| `UnchainedPlugin::resolvers::messages::kismet_log` | Blueprint execution messages; requires a build with `kismet_log`. |
| `UnchainedPlugin::resolvers::unchained_integration` | Engine startup, game-thread jobs, map loading, and match state. |
| `UnchainedPlugin::scan` | Signature scanning, detected game builds, and discovered offsets. |
| `UnchainedPlugin::seh` | Windows exception diagnostics and stack traces. |
| `UnchainedPlugin::tools::hook_globals` | Game integration initialization and TRACE dumps of its state. |
| `UnchainedPlugin::logging::log4rs` | Rust panic reports and backtraces after logging starts. |
| `UnchainedPlugin::tools::pdb_scan` | Function-symbol lookups; requires a build with `with_pdb`. |

Older custom configurations using `function`, `RCON`, `game_chat`, or
`system_chat` must use the full namespace names above instead.

## Troubleshooting

- **No TRACE in the console:** enable `trace` for the target and lower the console
  threshold to `trace`. Restart, then reproduce the action.
- **A custom file prevents startup:** check startup stderr for path or YAML
  errors. An unreadable or invalid file does not fall back to defaults. Remove
  `--log4rs-yaml` and restart to restore the embedded configuration.
- **No file output:** check the server account, saved-directory suffix, and write
  permissions. If `%LOCALAPPDATA%` is unavailable, the default directory falls
  back to `Logs` under the process's working directory.
- **Too much output:** set the relevant namespace to `info` or `warn`. Leave
  dependency filters in place when diagnosing a plugin module.

Diagnostics emitted before logging initializes are not controlled by these
namespace settings.

Logs can include player names, network addresses, command arguments, and
credentials in diagnostic output. Review and redact logs before sharing them;
enable detailed logging only where needed.

## Contributor guide: emitting diagnostics

Use `sinfo!`, `swarn!`, `serror!`, `sdebug!`, and `strace!` for plugin
diagnostics, including hooks. The logging module brings them into scope before
other plugin modules. Inside declarative macro definitions use `$crate::sinfo!`
and the corresponding severities; external binaries import from `UnchainedPlugin`.
Proc-generated diagnostics retain `crate::` paths for consumer resolution without
depending on lexical macro imports.

The macros capture `enclosing_function` in the attributes for structured logging
and the `{fn}` log format string.

Separate inline segments with commas; group each format string and its arguments:

```rust
sinfo!(("some log segment"), palette("warn"), ("next segment {}", count), palette(), ("normal text"));
```

Ordinary calls such as `sinfo!("count {}", count)` are unchanged. Use grouped
segments only for inline formatting; grouped calls use the inline record path
even without palette switches. Existing `fn;` and structured-field prefixes remain.

`palette(reference)` accepts a `&str` naming a configured color, ignoring ASCII
case. `palette()` restores normal console text, not the previous body color.
Unknown names leave the current color unchanged. Switches last until the next
switch or end of the record. Use an unpadded, untruncated `{m}` in the palette
encoder's pattern; buffering formatters such as `{h({m})}` are not supported for
inline colors. File, JSON, and redirected output concatenate the text segments
without adding color codes. JSON attributes include an internal
`__unchained_inline` record identifier.

Segments use stack-backed formatting arguments. Disabled records evaluate no
segment arguments; enabled records do not allocate or parse message text for
palette switches. Colors are prepared at startup, including colors referenced
only by inline messages.

The color-enabled palette encoder checks each record for `__unchained_inline`,
including records without switches. Ordinary records incur that metadata lookup
but do not enter the inline writer's scoped context. File and redirected output
return through the plain encoder before this check. No modified log4rs is required.

For other appenders, routing, and format options, see the
[log4rs configuration reference](https://docs.rs/log4rs/latest/log4rs/config/index.html).
