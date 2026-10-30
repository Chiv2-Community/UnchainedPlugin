use std::collections::BTreeMap;
use log::{Level, Record};
use log4rs::config::{Deserialize, Deserializers};
use log4rs::encode::{writer::simple::SimpleWriter, Color, Encode, Style, Write};

use super::pattern::PatternEncoder;

scoped_tls_hkt::scoped_thread_local!(static INLINE: for<'a> (&'a PaletteEncoder, &'a dyn InlineSink, u64));

pub(crate) fn inline_color(id: u64, name: &str) {
    if !INLINE.is_set() { return; }
    INLINE.with(|(encoder, output, current)| {
        if id != current { return; }
        let index = if name.is_empty() { None } else {
            let Ok(position) = encoder.names.binary_search_by(|(key, _)|
                key.bytes().cmp(name.bytes().map(|byte| byte.to_ascii_lowercase()))) else {
                return;
            };
            encoder.names[position].1
        };
        output.set_color(encoder, index);
    })
}

pub(crate) const DEFAULT_PATTERN: &str = "{palette({l})}[{d(%H:%M:%S)} {l:5}]{palette({l}_body)} [{sm:<-24.24}] {m}{n}";

#[derive(Debug, Clone, Copy)]
struct Rgb([u8; 3]);

impl<'de> serde::Deserialize<'de> for Rgb {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        let hex = value.strip_prefix('#').filter(|hex| hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .ok_or_else(|| serde::de::Error::custom("expected a quoted #RRGGBB color or null"))?;
        let mut rgb = [0; 3];
        for (i, channel) in rgb.iter_mut().enumerate() {
            *channel = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
                .map_err(serde::de::Error::custom)?;
        }
        Ok(Self(rgb))
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaletteEncoderConfig {
    pattern: Option<String>,
    palette: Option<BTreeMap<String, Option<Rgb>>>,
}

#[derive(Debug)]
enum Chunk {
    Text(PatternEncoder),
    Color([Option<usize>; 5]),
}

#[derive(Debug)]
struct Entry {
    rgb: [u8; 3],
    ansi: String,
    style: Style,
}

#[derive(Debug)]
pub(crate) struct PaletteEncoder {
    plain: PatternEncoder,
    chunks: Vec<Chunk>,
    colors: Vec<Entry>,
    names: Vec<(String, Option<usize>)>,
    truecolor: bool,
    enabled: bool,
}

impl PaletteEncoderConfig {
    pub(crate) fn build(self) -> anyhow::Result<PaletteEncoder> {
        let palette = self.palette.unwrap_or_else(|| {
            [("error", Some([242, 139, 130])), ("warn", Some([242, 204, 120])),
                ("info", Some([140, 180, 225])), ("debug", Some([128, 128, 128])),
                ("trace", Some([92, 92, 92])), ("error_body", Some([238, 238, 238])), ("warn_body", Some([238, 238, 238])),
                ("info_body", Some([238, 238, 238])), ("debug_body", Some([128, 128, 128])),
                ("trace_body", Some([92, 92, 92])), ("title", Some([255, 255, 0])),
                ("subtitle", Some([255, 0, 0]))]
                .into_iter().map(|(name, rgb)| (name.to_owned(), rgb.map(Rgb))).collect()
        });
        let mut entries = BTreeMap::new();
        let mut colors = Vec::new();
        let mut palette: Vec<_> = palette.into_iter()
            .map(|(name, rgb)| (name.to_ascii_lowercase(), rgb)).collect();
        // Keep the default legacy slots stable for embedded message color codes.
        palette.sort_by_key(|(name, _)| ["error", "warn", "info", "debug", "trace"]
            .iter().position(|severity| name == severity).unwrap_or(5));
        for (name, rgb) in palette {
            anyhow::ensure!(!name.is_empty(), "Palette names must not be empty");
            anyhow::ensure!(!entries.contains_key(&name), "Duplicate palette name: {name}");
            let index = rgb.map(|Rgb(rgb)| {
                if let Some(index) = colors.iter().position(|color| *color == rgb) { return index; }
                colors.push(rgb);
                colors.len() - 1
            });
            entries.insert(name, index);
        }
        let (chunks, plain) = parse(self.pattern.as_deref().unwrap_or(DEFAULT_PATTERN), &entries)?;
        // Inline messages can reference any configured entry, not just pattern colors.
        let colors = colors.into_iter().map(|rgb| {
            let [r, g, b] = rgb;
            Entry { rgb, ansi: format!("\x1b[0;38;2;{r};{g};{b}m"), style: Style::new() }
        }).collect();
        Ok(PaletteEncoder {
            plain: PatternEncoder::new(&plain),
            chunks,
            colors,
            names: entries.into_iter().collect(),
            truecolor: false,
            enabled: false,
        })
    }
}

fn escaped(bytes: &[u8], i: usize) -> bool {
    bytes[i] == b'\\' || (matches!(bytes[i], b'{' | b'}' | b'(' | b')')
        && bytes.get(i + 1) == Some(&bytes[i]))
}

fn format_fill(pattern: &str, i: usize) -> usize {
    if pattern.as_bytes().get(i) != Some(&b':') { return i; }
    let mut chars = pattern[i + 1..].chars();
    match (chars.next(), chars.next()) {
        (Some(fill), Some('<' | '>')) => i + 1 + fill.len_utf8() + 1,
        _ => i + 1,
    }
}

fn formatter_head(pattern: &str, i: usize) -> usize {
    let mut end = i + 1;
    for ch in pattern[end..].chars() {
        if !(if end == i + 1 { ch.is_alphabetic() } else { ch.is_alphanumeric() }) { break; }
        end += ch.len_utf8();
    }
    // Native format-spec fills are raw characters, not brace/backslash escapes.
    format_fill(pattern, end)
}

fn resolve(name: &str, palette: &BTreeMap<String, Option<usize>>) -> anyhow::Result<[Option<usize>; 5]> {
    if name.is_empty() { return Ok([None; 5]); }
    let bytes = name.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if escaped(bytes, i) { i = (i + 2).min(bytes.len()); continue; }
        if bytes[i] == b'{' {
            let head = formatter_head(name, i);
            let end = bytes[head..].iter().position(|byte| *byte == b'}')
                .ok_or_else(|| anyhow::anyhow!("Unclosed palette name formatter"))? + head;
            let formatter = name[i + 1..end].split(':').next().unwrap();
            anyhow::ensure!(matches!(formatter, "l" | "level"),
                "Palette names support only {{l}} and {{level}} interpolation");
            i = end;
        }
        i += 1;
    }
    let pattern = PatternEncoder::new(name);
    // Resolve severity-dependent names once; encoding uses only indexes.
    let mut indexes = [None; 5];
    for (i, level) in [Level::Error, Level::Warn, Level::Info, Level::Debug, Level::Trace].into_iter().enumerate() {
        let mut key = Vec::new();
        pattern.encode(&mut SimpleWriter(&mut key), &Record::builder().level(level).build())?;
        key.make_ascii_lowercase();
        let key = std::str::from_utf8(&key)?;
        indexes[i] = *palette.get(key).ok_or_else(|| anyhow::anyhow!("Unknown palette color: {key}"))?;
    }
    Ok(indexes)
}

fn parse(pattern: &str, palette: &BTreeMap<String, Option<usize>>) -> anyhow::Result<(Vec<Chunk>, String)> {
    let bytes = pattern.as_bytes();
    let mut chunks = Vec::new();
    let mut plain = String::new();
    let mut start = 0;
    let mut i = 0;
    let mut depth: usize = 0;
    while i < bytes.len() {
        if escaped(bytes, i) { i = (i + 2).min(bytes.len()); continue; }
        if bytes[i..].starts_with(b"{palette(") {
            anyhow::ensure!(depth == 0, "Palette directives must be outside other formatters");
            if start < i {
                plain.push_str(&pattern[start..i]);
                chunks.push(Chunk::Text(PatternEncoder::new(&pattern[start..i])));
            }
            let name_start = i + "{palette(".len();
            i = name_start;
            let mut braces = 1;
            while i < bytes.len() {
                // Leave following literal-brace escapes to the text encoder.
                if braces == 1 && bytes[i..].starts_with(b")}") {
                    i += 1;
                    break;
                }
                if escaped(bytes, i) { i = (i + 2).min(bytes.len()); continue; }
                match bytes[i] {
                    b'{' => { braces += 1; i = formatter_head(pattern, i); continue; }
                    b'}' => { braces -= 1; if braces == 0 { break; } }
                    b')' => { i = format_fill(pattern, i + 1); continue; }
                    _ => {}
                }
                i += 1;
            }
            anyhow::ensure!(i < bytes.len() && i > name_start && bytes[i - 1] == b')',
                "Expected {{palette(name)}} or {{palette()}}");
            let name = &pattern[name_start..i - 1];
            chunks.push(Chunk::Color(resolve(name, palette)?));
            i += 1;
            start = i;
            continue;
        }
        match bytes[i] {
            b'{' => { depth += 1; i = formatter_head(pattern, i); continue; }
            b'}' => depth = depth.saturating_sub(1),
            b')' if depth > 0 => { i = format_fill(pattern, i + 1); continue; }
            _ => {}
        }
        i += 1;
    }
    if start < pattern.len() {
        plain.push_str(&pattern[start..]);
        chunks.push(Chunk::Text(PatternEncoder::new(&pattern[start..])));
    }
    Ok((chunks, plain))
}

impl PaletteEncoder {
    pub(super) fn configure(&mut self, target: log4rs::append::console::Target) {
        if self.colors.is_empty() && !self.chunks.iter().any(|chunk| matches!(chunk, Chunk::Color(_))) { return; }
        let colors: Vec<_> = self.colors.iter().map(|entry| entry.rgb).collect();
        let Some((truecolor, slots)) = crate::logging::console::configure_palette(target, &colors) else { return };
        self.enabled = true;
        self.truecolor = truecolor;
        for (entry, slot) in self.colors.iter_mut().zip(slots) {
            let color = match slot & 7 {
                0 => Color::Black, 1 => Color::Blue, 2 => Color::Green, 3 => Color::Cyan,
                4 => Color::Red, 5 => Color::Magenta, 6 => Color::Yellow, _ => Color::White,
            };
            entry.style.text(color).intense(slot & 8 != 0);
        }
    }
}

impl Encode for PaletteEncoder {
    fn encode(&self, writer: &mut dyn Write, record: &Record) -> anyhow::Result<()> {
        // Redirected and file output use the ordinary pattern encoder directly.
        if !self.enabled { return self.plain.encode(writer, record); }
        let inline = record.key_values().get("__unchained_inline".into()).and_then(|value| value.to_u64());
        let result = if let Some(id) = inline {
            let output = InlineOutput { writer: std::cell::RefCell::new(writer), error: std::cell::RefCell::new(None) };
            let result = INLINE.set((self, &output, id), || self.encode_chunks(&mut &output, record));
            let error = output.error.borrow_mut().take();
            result.and_then(|()| error.map_or(Ok(()), |error| Err(error.into())))
        } else {
            self.encode_chunks(writer, record)
        };
        let reset = writer.set_style(&Style::new());
        result?;
        reset?;
        Ok(())
    }
}

impl PaletteEncoder {
    fn set_color(&self, writer: &mut dyn Write, index: Option<usize>) -> std::io::Result<()> {
        if let Some(index) = index {
            let entry = &self.colors[index];
            writer.set_style(&entry.style)?;
            if self.truecolor { writer.write_all(entry.ansi.as_bytes())?; }
        } else {
            writer.set_style(&Style::new())?;
        }
        Ok(())
    }

    fn encode_chunks(&self, writer: &mut dyn Write, record: &Record) -> anyhow::Result<()> {
        let level = record.level() as usize - 1;
        for chunk in &self.chunks {
            match chunk {
                Chunk::Text(pattern) => pattern.encode(writer, record)?,
                Chunk::Color(indexes) => self.set_color(writer, indexes[level])?,
            }
        }
        Ok(())
    }
}

// Formatting writes through this shared proxy so zero-width switches can change
// the writer's native style without embedding control codes in the message.
struct InlineOutput<'a> {
    writer: std::cell::RefCell<&'a mut dyn Write>,
    error: std::cell::RefCell<Option<std::io::Error>>,
}

trait InlineSink {
    fn set_color(&self, encoder: &PaletteEncoder, index: Option<usize>);
}

impl InlineSink for InlineOutput<'_> {
    fn set_color(&self, encoder: &PaletteEncoder, index: Option<usize>) {
        if self.error.borrow().is_none() {
            if let Err(error) = encoder.set_color(&mut **self.writer.borrow_mut(), index) {
                *self.error.borrow_mut() = Some(error);
            }
        }
    }
}

impl std::io::Write for &InlineOutput<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if let Some(error) = self.error.borrow_mut().take() { return Err(error); }
        self.writer.borrow_mut().write(bytes)
    }
    fn flush(&mut self) -> std::io::Result<()> { self.writer.borrow_mut().flush() }
}

impl Write for &InlineOutput<'_> {
    fn set_style(&mut self, style: &Style) -> std::io::Result<()> {
        if let Some(error) = self.error.borrow_mut().take() { return Err(error); }
        self.writer.borrow_mut().set_style(style)
    }
}

pub struct PaletteEncoderDeserializer;

impl Deserialize for PaletteEncoderDeserializer {
    type Trait = dyn Encode;
    type Config = PaletteEncoderConfig;

    fn deserialize(&self, config: PaletteEncoderConfig, _: &Deserializers)
        -> anyhow::Result<Box<dyn Encode>> {
        Ok(Box::new(config.build()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    fn encoder(yaml: &str) -> PaletteEncoder {
        serde_yaml::from_str::<PaletteEncoderConfig>(yaml).unwrap().build().unwrap()
    }

    #[derive(Default)]
    struct StyledBuffer {
        bytes: Vec<u8>,
        styles: Vec<(usize, Style)>,
        fail_after: Option<usize>,
    }

    impl io::Write for StyledBuffer {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let count = self.fail_after.map_or(bytes.len(), |limit| limit.saturating_sub(self.bytes.len()).min(bytes.len()));
            if count == 0 && !bytes.is_empty() { return Err(io::Error::other("injected output failure")); }
            self.bytes.extend_from_slice(&bytes[..count]);
            Ok(count)
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }

    impl Write for StyledBuffer {
        fn set_style(&mut self, style: &Style) -> io::Result<()> {
            self.styles.push((self.bytes.len(), style.clone()));
            Ok(())
        }
    }

    #[test]
    fn names_are_resolved_at_startup_for_all_levels_and_repeated_switches() {
        let mut encoder = encoder(r##"
pattern: "{palette(text)}[{palette({l})}{l:5}{palette({level}_body)}] [{sm}] {fn}|{K(request)}|{m}{palette()}"
palette:
  text: "#eeeeee"
  error: "#f28b82"
  warn: "#f2cc78"
  info: "#8cb4e1"
  debug: "#808080"
  trace: "#5c5c5c"
  error_body: null
  warn_body: null
  info_body: null
  debug_body: "#808080"
  trace_body: "#5c5c5c"
"##);
        assert_eq!(encoder.colors.len(), 6, "duplicate RGB values share an entry");
        for level in [Level::Error, Level::Warn, Level::Info, Level::Debug, Level::Trace] {
            let values = [("enclosing_function", "named"), ("request", "42")];
            let record = Record::builder().level(level).module_path(Some("UnchainedPlugin::messages"))
                .key_values(&values).args(format_args!("hello 世界 {{palette(text)}}")).build();
            let mut bytes = Vec::new();
            encoder.encode(&mut SimpleWriter(&mut bytes), &record).unwrap();
            assert_eq!(String::from_utf8(bytes).unwrap(),
                format!("[{level:5}] [UP::messages] named|42|hello 世界 {{palette(text)}}"));
            encoder.enabled = true;
            encoder.truecolor = true;
            let mut bytes = Vec::new();
            encoder.encode(&mut log4rs::encode::writer::ansi::AnsiWriter(&mut bytes), &record).unwrap();
            let text = String::from_utf8(bytes).unwrap();
            assert!(text.contains("\x1b[0;38;2;238;238;238m"));
            assert!(text.ends_with("\x1b[0m"));
            encoder.truecolor = false;
            encoder.enabled = false;
        }
    }

    #[test]
    fn defaults_preserve_header_and_body_colors_and_embedded_message_codes() {
        let mut encoder = encoder("{}");
        encoder.enabled = true;
        encoder.truecolor = true;
        for (level, rgb) in [(Level::Error, "242;139;130"), (Level::Warn, "242;204;120"),
            (Level::Info, "140;180;225"), (Level::Debug, "128;128;128"), (Level::Trace, "92;92;92")] {
            let record = Record::builder().level(level).module_path(Some("plugin::module"))
                .args(format_args!("before \x1b[31mred\x1b[0m after")).build();
            let mut bytes = Vec::new();
            encoder.encode(&mut log4rs::encode::writer::ansi::AnsiWriter(&mut bytes), &record).unwrap();
            let text = String::from_utf8(bytes).unwrap();
            let color = text.find(&format!("\x1b[0;38;2;{rgb}m")).unwrap();
            let body = text.find(" [p::module               ]").unwrap();
            let body_rgb = if level <= Level::Info { "238;238;238" } else { rgb };
            assert!(text[color + 1..body].contains(&format!("\x1b[0;38;2;{body_rgb}m")));
            assert!(text.contains("before \x1b[31mred\x1b[0m after"));
            assert!(text.ends_with("\x1b[0m"));
        }
    }

    #[test]
    fn arbitrary_names_switch_legacy_styles_and_reset_after_encoding_errors() {
        let mut encoder = encoder("pattern: '{palette(accent)}{m}{palette(normal)}end'\npalette: {ACCENT: '#123456', normal: null}");
        encoder.enabled = true;
        encoder.colors[0].style.text(Color::Magenta).intense(true);
        let record = Record::builder().args(format_args!("hello")).build();
        let mut buffer = StyledBuffer::default();
        encoder.encode(&mut buffer, &record).unwrap();
        assert_eq!(buffer.bytes, b"helloend");
        assert_eq!(buffer.styles[0].1.text, Some(Color::Magenta));
        assert_eq!(buffer.styles[0].1.intense, Some(true));
        assert_eq!(buffer.styles[1], (5, Style::new()));
        for rgb in [false, true] {
            encoder.truecolor = rgb;
            for limit in [0, 3, 6] {
                let mut buffer = StyledBuffer { fail_after: Some(limit), ..Default::default() };
                let error = encoder.encode(&mut buffer, &record).unwrap_err();
                assert!(error.to_string().contains("injected output failure"));
                assert_eq!(buffer.styles.last().unwrap().1, Style::new());
            }
        }
    }

    #[test]
    fn pattern_unused_colors_remain_available_to_inline_messages() {
        let mut table = BTreeMap::new();
        for i in 0..64 { table.insert(format!("color{i}"), Some(Rgb([i, i, i]))); }
        let encoder = PaletteEncoderConfig {
            pattern: Some("{palette(color63)}hello{palette()}".to_owned()), palette: Some(table),
        }.build().unwrap();
        assert_eq!(encoder.colors.len(), 64);
    }

    #[test]
    fn plain_output_skips_all_color_switches() {
        let encoder = encoder("pattern: '{palette(error)}{l}|{m}{palette()}'");
        let mut buffer = StyledBuffer::default();
        encoder.encode(&mut buffer, &Record::builder().level(Level::Info)
            .args(format_args!("message")).build()).unwrap();
        assert_eq!(buffer.bytes, b"INFO|message");
        assert!(buffer.styles.is_empty());
    }

    #[test]
    fn ordinary_records_check_inline_metadata_without_entering_inline_scope() {
        struct Lookups(std::cell::Cell<usize>);
        impl log::kv::Source for Lookups {
            fn visit<'kvs>(&'kvs self, _: &mut dyn log::kv::VisitSource<'kvs>) -> Result<(), log::kv::Error> {
                panic!("ordinary encoder enumerated metadata")
            }
            fn get(&self, key: log::kv::Key<'_>) -> Option<log::kv::Value<'_>> {
                assert_eq!(key.as_str(), "__unchained_inline");
                self.0.set(self.0.get() + 1);
                None
            }
        }
        let lookups = Lookups(std::cell::Cell::new(0));
        let mut encoder = encoder("pattern: '{palette(info)}{m}'");
        encoder.enabled = true;
        let mut output = StyledBuffer::default();
        encoder.encode(&mut output, &Record::builder().key_values(&lookups)
            .args(format_args!("ordinary {}", 7)).build()).unwrap();
        assert_eq!(output.bytes, b"ordinary 7");
        assert_eq!(lookups.0.get(), 1);
        assert!(!INLINE.is_set());
    }

    #[test]
    fn inline_switches_stream_styles_without_changing_plain_or_json_messages() {
        use std::sync::Mutex;
        struct Logger { encoder: PaletteEncoder, output: Mutex<StyledBuffer> }
        impl log::Log for Logger {
            fn enabled(&self, _: &log::Metadata) -> bool { true }
            fn log(&self, record: &Record) {
                self.encoder.encode(&mut *self.output.lock().unwrap(), record).unwrap();
                assert_eq!(record.args().to_string(), "before 7after 世界normalend");
                let mut plain = Vec::new();
                PatternEncoder::new("{m}").encode(&mut SimpleWriter(&mut plain), record).unwrap();
                assert_eq!(plain, b"before 7after \xe4\xb8\x96\xe7\x95\x8cnormalend");
                let mut json = Vec::new();
                log4rs::encode::json::JsonEncoder::new().encode(&mut SimpleWriter(&mut json), record).unwrap();
                let json: serde_json::Value = serde_json::from_slice(&json).unwrap();
                assert_eq!(json["message"], "before 7after 世界normalend");
            }
            fn flush(&self) {}
        }
        log::set_max_level(log::LevelFilter::Trace);
        for truecolor in [false, true] {
            let mut encoder = encoder("pattern: '{m}|{m}'\npalette: {accent: '#123456', normal: null}");
            encoder.enabled = true;
            encoder.truecolor = truecolor;
            encoder.colors[0].style.text(Color::Magenta);
            let logger = Logger { encoder, output: Mutex::new(StyledBuffer::default()) };
            let reference = String::from("AcCeNt");
            sinfo!(logger: &logger, ("before {}", 7), palette(&reference), ("after {}", "世界"),
                palette("normal"), "normal", palette("unknown"), "end", palette());
            let output = logger.output.lock().unwrap();
            let message = if truecolor { "before 7\x1b[0;38;2;18;52;86mafter 世界normalend" }
                else { "before 7after 世界normalend" };
            assert_eq!(output.bytes, format!("{message}|{message}").as_bytes());
            assert_eq!(output.styles[0].0, 8);
            assert_eq!(output.styles[0].1.text, Some(Color::Magenta));
            assert_eq!(output.styles[1].1, Style::new());
            assert_eq!(output.styles.last().unwrap().1, Style::new());
        }
    }

    #[test]
    fn inline_output_errors_reset_style_and_release_scope() {
        use crate::logging::inline::{Message, Segment};
        let segments = [Segment::Text(format_args!("before")), Segment::Palette("warn"),
            Segment::Text(format_args!("after")), Segment::Palette("")];
        let values = [("__unchained_inline", 1u64)];
        let mut encoder = encoder("pattern: '{m}'");
        encoder.enabled = true;
        for truecolor in [false, true] {
            encoder.truecolor = truecolor;
            for limit in [0, 3, 7] {
                let mut output = StyledBuffer { fail_after: Some(limit), ..Default::default() };
                let error = encoder.encode(&mut output, &Record::builder().key_values(&values)
                    .args(format_args!("{}", Message(&segments, 1))).build()).unwrap_err();
                assert!(error.to_string().contains("injected output failure"));
                assert_eq!(output.styles.last().unwrap().1, Style::new());
                assert!(!INLINE.is_set());
            }
        }
    }

    #[test]
    fn nested_and_concurrent_inline_records_keep_separate_writers() {
        use std::{fmt, sync::Mutex};
        struct Logger { encoder: PaletteEncoder, output: Mutex<Vec<Vec<u8>>> }
        impl log::Log for Logger {
            fn enabled(&self, _: &log::Metadata) -> bool { true }
            fn log(&self, record: &Record) {
                let mut output = StyledBuffer::default();
                self.encoder.encode(&mut output, record).unwrap();
                self.output.lock().unwrap().push(output.bytes);
            }
            fn flush(&self) {}
        }
        struct Nested<'a>(&'a Logger);
        impl fmt::Display for Nested<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                sinfo!(logger: self.0, ("inner"), palette("warn"), ("yellow"));
                f.write_str("outer")
            }
        }
        log::set_max_level(log::LevelFilter::Trace);
        let mut encoder = encoder("pattern: '{m}'");
        encoder.enabled = true;
        encoder.truecolor = true;
        let logger = Logger { encoder, output: Mutex::new(Vec::new()) };
        std::thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| sinfo!(logger: &logger, ("{}", Nested(&logger)), palette("info"), ("blue")));
            }
        });
        let output = logger.output.lock().unwrap();
        assert_eq!(output.iter().filter(|line| *line == b"inner\x1b[0;38;2;242;204;120myellow").count(), 4);
        assert_eq!(output.iter().filter(|line| *line == b"outer\x1b[0;38;2;140;180;225mblue").count(), 4);
        assert!(!INLINE.is_set());
    }

    #[test]
    #[ignore = "manual microbenchmark; run in release mode with --nocapture"]
    fn inline_palette_overhead() {
        struct Logger(PaletteEncoder);
        impl log::Log for Logger {
            fn enabled(&self, _: &log::Metadata) -> bool { true }
            fn log(&self, record: &Record) {
                self.0.encode(&mut SimpleWriter(std::io::sink()), record).unwrap();
            }
            fn flush(&self) {}
        }
        log::set_max_level(log::LevelFilter::Trace);
        let mut encoder = encoder("pattern: '{m}'");
        encoder.enabled = true;
        let logger = Logger(encoder);
        let iterations = 500_000;
        for inline in [false, true] {
            let start = std::time::Instant::now();
            for i in 0..iterations {
                let count = std::hint::black_box(i);
                if inline {
                    sinfo!(logger: &logger, ("before {}", count), palette("warn"), ("after"), palette(), ("end"));
                } else {
                    sinfo!(logger: &logger, "before {}afterend", count);
                }
            }
            eprintln!("inline={inline}: {:.0} ns/record", start.elapsed().as_nanos() as f64 / iterations as f64);
        }
    }

    #[test]
    fn default_palette_matches_shipped_yaml_and_legacy_slot_order() {
        let mut yaml: serde_yaml::Value = serde_yaml::from_str(super::super::EMBEDDED_LOG4RS_YAML).unwrap();
        let block = &mut yaml["appenders"]["console"]["encoder"];
        block.as_mapping_mut().unwrap().remove(serde_yaml::Value::String("kind".into()));
        let shipped = serde_yaml::from_value::<PaletteEncoderConfig>(block.clone()).unwrap().build().unwrap();
        let expected = [[242, 139, 130], [242, 204, 120], [140, 180, 225], [128, 128, 128], [92, 92, 92], [238, 238, 238],
            [255, 0, 0], [255, 255, 0]];
        for encoder in [shipped, encoder("{}")] {
            assert_eq!(encoder.colors.iter().map(|entry| entry.rgb).collect::<Vec<_>>(), expected);
        }
    }
}
