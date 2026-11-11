# Logging

Logging uses [log4rs](https://docs.rs/log4rs/latest/log4rs/config/index.html).
Appenders choose destinations, encoders format records, and loggers select targets.

## Defaults

The plugin loads one configuration at startup, in this order:

1. The file passed with `--log4rs-yaml`.
2. `log4rs.yaml` in the process's working directory.
3. The [embedded configuration](../src/logging/log4rs/log4rs.default.yaml).

Copy the embedded configuration to customize it. Pass its path with
`--log4rs-yaml "C:\path\log4rs.yaml"` and restart after editing.
A configuration error is reported on stderr; startup pauses for five seconds,
then loads the embedded configuration.

The root level is `debug`. The diagnostic console appender has a `debug` threshold.
Chat loggers use `info`. Console timestamps use `HH:MM:SS`; text files use
ISO 8601 with a timezone offset, such as `2026-11-10T14:30:00-06:00`.
Both formats have second precision.

### Log location

Default files are `unchained.log`, `chat.log`, and `kismet.log` in the launched
profile's saved directory. Most users use:

```text
%LOCALAPPDATA%\Chivalry 2\Saved_unchained\Logs
```

The directory follows `--saveddirsuffix`. Server hosts can have several
`Saved_<suffix>` folders; use the suffix passed to the relevant server process.
For example, `--saveddirsuffix MyServer` uses:

```text
%LOCALAPPDATA%\Chivalry 2\Saved_MyServer\Logs
```

Use the Windows account running the process to resolve `%LOCALAPPDATA%`.
If that variable is unavailable, files use `Logs` in the working directory.

Custom configurations can use these path placeholders:

| Placeholder | Default file |
| --- | --- |
| `__UNCHAINED_LOG_PATH__` | `unchained.log` |
| `__CHAT_LOG_PATH__` | `chat.log` |
| `__KISMET_LOG_PATH__` | `kismet.log` |

### Stdio

`--attach-stdio` uses inherited stdout/stderr or attaches to the parent console.
The calling program supplies any pipes or file redirection.
`--no-spawn-console` suppresses console allocation independently.

## Chat Logging

Chat records are written at `INFO` to `chat.log` in the launched profile's
[log directory](#log-location). Most users will find it at:

```text
%LOCALAPPDATA%\Chivalry 2\Saved_unchained\Logs\chat.log
```

### Custom chat encoder

Custom YAML replaces the entire default configuration. Start with a copy of the
[embedded configuration](../src/logging/log4rs/log4rs.default.yaml).
Save it as `log4rs.yaml` in the Chivalry 2 process's working directory, or use
`--log4rs-yaml chat-logging.yaml` for another file in that directory.

This fragment shows only the chat sections. Merge them into your copied configuration,
retaining the other appenders, loggers, and root settings:

```yaml
appenders:
  chat:
    kind: file
    path: "__CHAT_LOG_PATH__"
    encoder:
      kind: pattern
      pattern: "{d(%Y-%m-%dT%H:%M:%S%:z)} | {K(label):<9} | {K(sender):<16} - {m}{n}"
loggers:
  chat_log:
    level: info
    appenders:
      - chat
    additive: false
  "chat_log::system":
    level: info
    appenders:
      - chat
    additive: false
  "chat_log::system::sender":
    level: info
    appenders:
      - chat
    additive: false
```

The `chat_log` logger routes chat and its child targets to the `chat` appender.
`additive: false` keeps this human-readable stream separate from root output.
This custom pattern includes the sender field for every record; senderless system
records leave it empty.

### Targets

| Target | Records |
| --- | --- |
| `chat_log` | Other native player channels; parent for all chat targets. |
| `chat_log::all` | Native global player chat and known-sender AllSay system chat. |
| `chat_log::team` | Native TeamSay player chat. |
| `chat_log::system` | Senderless native system chat. |
| `chat_log::system::sender` | Other native system chat with a known sender. |
| `chat_log::external` | Discord chat. |

### Attributes

Chat attributes are available through `{K(name)}` in patterns and under `attributes`
in JSON records:

| Attribute | Value |
| --- | --- |
| `chat` | Channel, such as `AllSay`, `TeamSay`, `Spectator`, `ServerSay`, or Discord's `Global`. |
| `sender` | Sender name; empty for senderless system records. |
| `source` | `Game` or `Discord`. |
| `chat_kind` | `player`, `system`, or `external`. |
| `label` | Uppercase display label, such as `ALL`, `MASON`, `SPECTATOR`, `SYSTEM`, or `DISCORD`. |
| `faction` | Native faction (`Agatha`, `Mason`, `Tenosia`, `FFA`, `None`, `All`, `MAX`, `Unknown`), `System`, or `Discord`. |

## Structured logging

Select the [log4rs JSON encoder](https://docs.rs/log4rs/latest/log4rs/encode/json/index.html)
on each appender that should write structured records:

```yaml
encoder:
  kind: json
```

Each record is one JSON line with log4rs fields and an `attributes` object for
plugin metadata. Chat records retain their raw bodies and full attribute values.
Root appenders with `kind: json` also receive isolated chat records once;
their filters still apply.

See [chat targets](#targets) and [chat attributes](#attributes) for routing and metadata.

## Format log lines

The plugin extends the [log4rs pattern syntax](https://docs.rs/log4rs/latest/log4rs/encode/pattern/index.html)
with these fields:

| Field | Value |
| --- | --- |
| `{fn}` | Enclosing function's leaf name. |
| `{sm}` | Abbreviated source module, limited to its last 32 Unicode characters. |

`{fn}` removes the generated `_detour_fkt` suffix before padding and truncation.
Closures use their enclosing function. Missing function metadata displays `???`.
Defaults use `{fn:<16}`: left-aligned with a minimum width of 16 characters.
`{sm}` abbreviates parent segments by word initials:
`UnchainedPlugin::resolvers::messages` becomes `UP::r::messages`.
Both fields support log4rs nesting, escaping, alignment, and truncation.
The extensions apply to `pattern` and `palette` encoders in console, file, and
rolling-file appenders.

Plugin metadata is available through `{K(name)}` and JSON `attributes`:

| Attribute | Value |
| --- | --- |
| `enclosing_function` | Enclosing function name captured by plugin logging macros. |
| `inline_colors` | JSON-encoded palette switches with UTF-8 byte offsets; used by the palette encoder. |
| `undefined_palette` | Missing palette name reported by startup validation. |

Chat metadata is listed under [chat attributes](#attributes).

### Inline removals

Both encoders accept regexes that remove matches from the rendered line:

```yaml
encoder:
  kind: pattern
  inline-removals:
    - _detour_fkt
  pattern: "{fn:<16} - {m}{n}"
```

Expressions are combined with alternation and compiled once per encoder. Removal
runs after padding and truncation, across fields and color changes. Regex matching
is case-sensitive by default. Invalid expressions reject the configuration.
An empty list disables removal.

The default rule removes `_detour_fkt` elsewhere in diagnostic lines.
Raw records and JSON retain their original values. Redact credentials separately
before sharing logs.

### Palette entries

```yaml
encoder:
  kind: palette
  pattern: "{palette({l})}{l:<5}{palette()} {m}{n}"
  palette:
    error: "#f28b82"
    warn: "#f2cc78"
    info: "#8cb4e1"
    debug: "#808080"
    trace: "#5c5c5c"
```

`{palette(name)}` selects a color until the next switch or record end.
`{palette()}` restores normal text. Names are ASCII case-insensitive.
`{l}` or `{level}` selects a severity-specific entry.

| Entry | Effect |
| --- | --- |
| `accent: "#f2cc78"` | Foreground color with the default background. |
| `panel: {foreground: "#f2cc78", background: "#202020"}` | Foreground and background colors. |
| `panel: {background: "#202020"}` | Background color with the default foreground. |
| `accent: {foreground: "#f2cc78", background: inherit}` | Foreground color retaining the current background. |
| `normal: null` | Default terminal colors. |

Omitted or null object channels use terminal defaults; `inherit` retains the
current channel. Objects require at least one color or `inherit` value.
Explicit palettes replace the default table. Static names are validated at startup.
Dynamic names can use attributes, such as `{palette(team_{K(faction)})}`;
missing dynamic names restore normal text.
Place palette switches outside other formatters. Legacy consoles approximate RGB colors.

## Set log levels

Levels are `off`, `error`, `warn`, `info`, `debug`, and `trace`.
Each level includes higher severities. `off` disables the target.

Merge entries into the existing `loggers` mapping:

```yaml
loggers:
  "UnchainedPlugin::features::server_registration":
    level: info
  "UnchainedPlugin::modules::vote":
    level: warn
  "chat_log::team":
    level: off
```

Target names are case-sensitive. A setting covers descendants unless a more
specific logger overrides it. Use `{t}` to inspect full targets.
Set `UnchainedPlugin` to change plugin diagnostics; `root.level` also affects dependencies.
Chat uses the separate `chat_log` target family.

### Enable TRACE output

Set the desired target to `trace`:

```yaml
"UnchainedPlugin::resolvers::gameplay":
  level: trace
```

For console output, also change the existing console threshold to `trace`.
The default file appender accepts every enabled severity.
Restart and reproduce the relevant action.

## Namespace reference

Plugin diagnostic targets inherit `debug` from the default root.
Features emit records when enabled and exercised.

| Target or group | Records |
| --- | --- |
| `UnchainedPlugin` | Startup, initialization, patches, and hooks; parent for plugin diagnostics. |
| `UnchainedPlugin::commands` | Command argument parsing failures. |
| `UnchainedPlugin::events::broadcast_message::console` | Broadcast text at TRACE. |
| `UnchainedPlugin::features::discord` | Discord connections, HTTP delivery, rate limits, and outbound queues. |
| `UnchainedPlugin::features::mod_management` | Mod discovery, spawning, and settings. |
| `UnchainedPlugin::features::rcon` | RCON listener, commands, and connection failures. |
| `UnchainedPlugin::features::server_registration` | Server discovery, registration, authentication, and heartbeats. |
| `UnchainedPlugin::game` | Game-state access and engine helpers. |
| `UnchainedPlugin::logging::console` | Console attachment failures. |
| `UnchainedPlugin::logging::log4rs` | Configuration errors, palette warnings, panics, and console drop warnings. |
| `UnchainedPlugin::modules::command` | Command execution and generated help. |
| `UnchainedPlugin::modules::discord` | Discord module configuration. |
| `UnchainedPlugin::modules::game_info` | Requested game and player details. |
| `UnchainedPlugin::modules::mod_dump` | Mod-registry file output. |
| `UnchainedPlugin::modules::mod_list` | Mod-list scans. |
| `UnchainedPlugin::modules::vote` | Vote cancellation. |
| `UnchainedPlugin::modules::votes` | Vote actions. |
| `UnchainedPlugin::resolvers::admin_control` | Native administration commands. |
| `UnchainedPlugin::resolvers::asset_loading` | Save-game loading and saving. |
| `UnchainedPlugin::resolvers::asset_registry` | Asset and object lookups. |
| `UnchainedPlugin::resolvers::backend_hooks` | Ban checks, player connections, and departures. |
| `UnchainedPlugin::resolvers::config_overrides` | Profile selection in verbose builds. |
| `UnchainedPlugin::resolvers::desync` | Blocked events and dedicated-server compatibility. |
| `UnchainedPlugin::resolvers::etc_hooks` | Travel, game calls, and verbose override warnings. |
| `UnchainedPlugin::resolvers::gameplay` | Combat validation and damage/kill TRACE details. |
| `UnchainedPlugin::resolvers::getpost_requests` | Backend request handling. |
| `UnchainedPlugin::resolvers::messages` | Native console output and invalid native chat channels. |
| `UnchainedPlugin::resolvers::messages::kismet_log` | Blueprint execution messages in `kismet_log` builds. |
| `UnchainedPlugin::resolvers::unchained_integration` | Engine startup, game-thread jobs, maps, and match state. |
| `UnchainedPlugin::scan` | Signatures, builds, and discovered offsets. |
| `UnchainedPlugin::seh` | Windows exceptions and stack traces. |
| `UnchainedPlugin::tools::hook_globals` | Integration initialization and TRACE state dumps. |
| `UnchainedPlugin::tools::pdb_scan` | Symbol lookups in `with_pdb` builds. |

Chat metadata and routing are listed under [Chat Logging](#chat-logging).

## Troubleshooting

- **Missing console TRACE:** set both the target level and console threshold to `trace`.
- **Rejected configuration:** read stderr or `unchained.log`, fix the reported error, and restart.
- **File initialization failure:** check the process account, resolved path, and write permissions.
- **Excess output:** lower the relevant target to `info` or `warn` and retain dependency filters.

Startup records include CLI settings with passwords, tokens, and backend URLs
masked; INI and passthrough arguments are represented by counts.
Logs can contain player names, network addresses, command arguments, and credentials
from other diagnostics. Review and redact logs before sharing them.

## Contributor guide: emitting diagnostics

Use `sinfo!`, `swarn!`, `serror!`, `sdebug!`, and `strace!`.
The logging module brings these macros into scope before callers.
Declarative macros use `$crate::sinfo!` and corresponding severities;
external binaries import them from `UnchainedPlugin`.

Each macro captures the enclosing function for `{fn}` and the JSON attribute
`enclosing_function`. Add structured values before the semicolon:

```rust
sinfo!(mod_count = count; "Loaded {} mods", count);
```

### Inline color changes

`palette("name")` selects a configured color; `palette()` restores normal text.
Unknown inline names restore normal text and literal references produce startup warnings.
Start with a palette switch or grouped segment; group format strings with their arguments:

```rust
sinfo!(palette("warn"), "Warning", palette(), " normal text");
sinfo!(("Loaded {} mods", count), palette("info"), " from {directory}");
sinfo!(palette("error"), ("Failed: {}", error), palette(), " retrying");
```
