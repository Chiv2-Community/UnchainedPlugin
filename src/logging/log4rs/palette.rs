use std::collections::BTreeMap;
use std::io::Write as _;
use log::{Level, Record};
use log4rs::config::{Deserialize, Deserializers};
use log4rs::encode::{writer::simple::SimpleWriter, Color, Encode, Style, Write};

use super::pattern::PatternEncoder;
use crate::logging::inline::{ColorChange, COLORS_KEY};

pub(crate) const DEFAULT_PATTERN: &str = "{palette(timestamp)}{d(%H:%M:%S)}{palette(trace)} | {palette({l})}{l:>5}{palette(trace)} | {palette(module)}{fn:<32.32}{palette(timestamp)} - {palette({l}_body)}{m}{n}";

// Built-in chat patterns select these names from structured channel/faction fields.
inventory::submit! { crate::logging::inline::PaletteReference("team_agatha") }
inventory::submit! { crate::logging::inline::PaletteReference("team_mason") }
inventory::submit! { crate::logging::inline::PaletteReference("team_tenosia") }
inventory::submit! { crate::logging::inline::PaletteReference("team_ffa") }
inventory::submit! { crate::logging::inline::PaletteReference("team_none") }
inventory::submit! { crate::logging::inline::PaletteReference("team_system") }
inventory::submit! { crate::logging::inline::PaletteReference("team_discord") }
inventory::submit! { crate::logging::inline::PaletteReference("team_unknown") }
inventory::submit! { crate::logging::inline::PaletteReference("team_max") }
inventory::submit! { crate::logging::inline::PaletteReference("team_all") }
inventory::submit! { crate::logging::inline::PaletteReference("team_spectator") }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Channel {
    Rgb([u8; 3]),
    Inherit,
}

impl<'de> serde::Deserialize<'de> for Channel {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        if value == "inherit" { return Ok(Self::Inherit); }
        let hex = value.strip_prefix('#').filter(|hex| hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .ok_or_else(|| serde::de::Error::custom("expected a quoted #RRGGBB color or inherit"))?;
        let mut rgb = [0; 3];
        for (i, channel) in rgb.iter_mut().enumerate() {
            *channel = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16)
                .map_err(serde::de::Error::custom)?;
        }
        Ok(Self::Rgb(rgb))
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Channels {
    foreground: Option<Channel>,
    background: Option<Channel>,
}

impl Channels {
    fn inherits(self) -> bool {
        self.foreground == Some(Channel::Inherit) || self.background == Some(Channel::Inherit)
    }
}

#[derive(serde::Deserialize)]
#[serde(untagged)]
enum PaletteColor {
    Foreground(Channel),
    Channels(Channels),
}

impl PaletteColor {
    fn channels(self) -> anyhow::Result<Channels> {
        match self {
            Self::Foreground(rgb) => Ok(Channels { foreground: Some(rgb), background: None }),
            Self::Channels(channels) => {
                anyhow::ensure!(channels.foreground.is_some() || channels.background.is_some(),
                    "Palette objects require a foreground or background channel");
                Ok(channels)
            }
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaletteEncoderConfig {
    pattern: Option<String>,
    palette: Option<BTreeMap<String, Option<PaletteColor>>>,
    #[serde(default, rename = "inline-removals")]
    inline_removals: Vec<String>,
}

#[derive(Debug)]
enum Chunk {
    Text(PatternEncoder),
    Message,
    Color([Option<usize>; 5]),
    DynamicColor(PatternEncoder),
}

#[derive(Debug)]
struct Entry {
    channels: Channels,
    ansi: String,
    style: Style,
}

#[derive(Debug)]
pub(crate) struct PaletteEncoder {
    inline_removals: Option<regex::Regex>,
    plain: PatternEncoder,
    chunks: Vec<Chunk>,
    colors: Vec<Entry>,
    names: Vec<(String, Option<usize>)>,
    truecolor: bool,
    enabled: bool,
}

impl PaletteEncoderConfig {
    pub(super) fn default_names() -> impl Iterator<Item = String> {
        Self::default_palette().into_keys()
    }

    fn default_palette() -> BTreeMap<String, Option<PaletteColor>> {
        let mut palette: BTreeMap<_, _> = [("error", Some([242, 139, 130])), ("warn", Some([242, 204, 120])),
            ("info", Some([140, 180, 225])), ("debug", Some([128, 128, 128])),
            ("trace", Some([92, 92, 92])), ("error_body", Some([238, 238, 238])), ("warn_body", Some([238, 238, 238])),
            ("info_body", Some([238, 238, 238])), ("debug_body", Some([128, 128, 128])),
            ("trace_body", Some([92, 92, 92])), ("timestamp", Some([144, 144, 144])),
            ("module", Some([190, 157, 92])), ("chat_all", Some([242, 179, 120])),
            ("team_discord", Some([181, 140, 255])), ("team_system", Some([140, 216, 225])),
            ("team_agatha", Some([140, 180, 225])), ("team_mason", Some([242, 139, 130])),
            ("team_tenosia", Some([208, 145, 40])), ("chat_server", Some([140, 216, 225])),
            ("team_ffa", Some([168, 213, 162])), ("team_unknown", Some([215, 160, 223])),
            ("team_none", Some([255, 255, 255])), ("team_max", Some([215, 160, 223])),
            ("team_all", Some([168, 213, 162])), ("team_spectator", Some([255, 255, 255]))]
            .into_iter().map(|(name, rgb)| (name.to_owned(), rgb.map(|rgb| PaletteColor::Foreground(Channel::Rgb(rgb))))).collect();
        for name in ["title", "subtitle"] {
            palette.insert(name.to_owned(), Some(PaletteColor::Channels(Channels {
                foreground: Some(Channel::Rgb([208, 145, 40])),
                background: Some(Channel::Rgb([47, 47, 47])),
            })));
        }
        palette
    }

    pub(crate) fn build(self) -> anyhow::Result<PaletteEncoder> {
        let palette = self.palette.unwrap_or_else(Self::default_palette);
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
            let index = rgb.map(PaletteColor::channels).transpose()?.map(|rgb| {
                if let Some(index) = colors.iter().position(|color| *color == rgb) { return index; }
                colors.push(rgb);
                colors.len() - 1
            });
            entries.insert(name, index);
        }
        let (chunks, plain) = parse(self.pattern.as_deref().unwrap_or(DEFAULT_PATTERN), &entries)?;
        // Inline messages can reference any configured entry, not just pattern colors.
        let colors = colors.into_iter().map(|rgb| {
            let mut ansi = String::from("\x1b[");
            if !rgb.inherits() { ansi.push_str("0;"); }
            for (channel, color) in [(38, rgb.foreground), (48, rgb.background)] {
                use std::fmt::Write;
                match color {
                    Some(Channel::Rgb([r, g, b])) => write!(ansi, "{channel};2;{r};{g};{b};").unwrap(),
                    None if rgb.inherits() => write!(ansi, "{};", channel + 1).unwrap(),
                    _ => {},
                }
            }
            if ansi.ends_with(';') { ansi.pop(); ansi.push('m'); }
            else { ansi.clear(); }
            Entry { channels: rgb, ansi, style: Style::new() }
        }).collect();
        Ok(PaletteEncoder {
            inline_removals: super::removals::compile(&self.inline_removals)?,
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

fn resolve(name: &str, palette: &BTreeMap<String, Option<usize>>) -> anyhow::Result<Chunk> {
    if name.is_empty() { return Ok(Chunk::Color([None; 5])); }
    let bytes = name.as_bytes();
    let mut i = 0;
    let mut dynamic = false;
    while i < bytes.len() {
        if escaped(bytes, i) { i = (i + 2).min(bytes.len()); continue; }
        if bytes[i] == b'{' {
            let head = formatter_head(name, i);
            let end = bytes[head..].iter().position(|byte| *byte == b'}')
                .ok_or_else(|| anyhow::anyhow!("Unclosed palette name formatter"))? + head;
            let formatter = name[i + 1..head].split(':').next().unwrap();
            anyhow::ensure!(matches!(formatter, "l" | "level" | "K"),
                "Palette names support level and structured-field interpolation");
            dynamic |= formatter == "K";
            i = end;
        }
        i += 1;
    }
    let pattern = PatternEncoder::new(name);
    if dynamic { return Ok(Chunk::DynamicColor(pattern)); }
    // Resolve severity-dependent names once; encoding uses only indexes.
    let mut indexes = [None; 5];
    for (i, level) in [Level::Error, Level::Warn, Level::Info, Level::Debug, Level::Trace].into_iter().enumerate() {
        let mut key = Vec::new();
        pattern.encode(&mut SimpleWriter(&mut key), &Record::builder().level(level).build())?;
        key.make_ascii_lowercase();
        let key = std::str::from_utf8(&key)?;
        indexes[i] = *palette.get(key).ok_or_else(|| anyhow::anyhow!("Unknown palette color: {key}"))?;
    }
    Ok(Chunk::Color(indexes))
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
        if depth == 0 {
            if let Some(token) = ["{m}", "{message}"].into_iter().find(|token| bytes[i..].starts_with(token.as_bytes())) {
                if start < i {
                    plain.push_str(&pattern[start..i]);
                    chunks.push(Chunk::Text(PatternEncoder::new(&pattern[start..i])));
                }
                plain.push_str(token);
                chunks.push(Chunk::Message);
                i += token.len();
                start = i;
                continue;
            }
        }
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
            chunks.push(resolve(name, palette)?);
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
        if self.colors.is_empty() && !self.chunks.iter().any(|chunk| matches!(chunk, Chunk::Color(_) | Chunk::DynamicColor(_))) { return; }
        let mut colors = Vec::new();
        for entry in &self.colors {
            for channel in [entry.channels.foreground, entry.channels.background] {
                if let Some(Channel::Rgb(rgb)) = channel {
                    if !colors.contains(&rgb) { colors.push(rgb); }
                }
            }
        }
        let Some((truecolor, slots)) = crate::logging::console::configure_palette(target, &colors) else { return };
        self.enabled = true;
        self.truecolor = truecolor;
        let backgrounds = crate::logging::console::background_slots(target, &colors);
        let color = |slot: u8| match slot & 7 {
                0 => Color::Black, 1 => Color::Blue, 2 => Color::Green, 3 => Color::Cyan,
                4 => Color::Red, 5 => Color::Magenta, 6 => Color::Yellow, _ => Color::White,
        };
        for entry in &mut self.colors {
            if let Some(Channel::Rgb(rgb)) = entry.channels.foreground {
                let slot = slots[colors.iter().position(|color| *color == rgb).unwrap()];
                entry.style.text(color(slot)).intense(slot & 8 != 0);
            }
            if let Some(Channel::Rgb(rgb)) = entry.channels.background {
                let slot = backgrounds[colors.iter().position(|color| *color == rgb).unwrap()];
                entry.style.background(color(slot));
            }
        }
    }
}

impl Encode for PaletteEncoder {
    fn encode(&self, writer: &mut dyn Write, record: &Record) -> anyhow::Result<()> {
        let result = if let Some(regex) = &self.inline_removals {
            let mut line = super::removals::RenderedLine::default();
            let rendered = if self.enabled {
                self.encode_chunks(&mut PaletteWriter { inner: writer, style: Style::new(), capture: Some(&mut line) }, record)
            } else {
                self.plain.encode(&mut line, record)
            };
            rendered.and_then(|()| line.emit(writer, regex))
        } else if self.enabled {
            self.encode_chunks(&mut PaletteWriter { inner: writer, style: Style::new(), capture: None }, record)
        } else {
            return self.plain.encode(writer, record);
        };
        let reset = if self.enabled { writer.set_style(&Style::new()) } else { Ok(()) };
        result?;
        reset?;
        Ok(())
    }
}

impl PaletteEncoder {
    fn set_color(&self, writer: &mut PaletteWriter<'_>, index: Option<usize>) -> std::io::Result<()> {
        if let Some(index) = index {
            let entry = &self.colors[index];
            if entry.channels.foreground == Some(Channel::Inherit) && entry.channels.background == Some(Channel::Inherit) {
                return Ok(());
            }
            if entry.channels.inherits() {
                if self.truecolor { return writer.ansi(entry.ansi.as_bytes()); }
                let mut style = entry.style.clone();
                if entry.channels.foreground == Some(Channel::Inherit) {
                    style.text = writer.style.text;
                    style.intense = writer.style.intense;
                }
                if entry.channels.background == Some(Channel::Inherit) {
                    style.background = writer.style.background;
                }
                return writer.set_style(&style);
            }
            if self.truecolor {
                // RGB codes replace the native fallback; avoid log4rs' small ANSI style buffer.
                writer.set_style(&Style::new())?;
                writer.ansi(entry.ansi.as_bytes())?;
            } else {
                writer.set_style(&entry.style)?;
            }
        } else {
            writer.set_style(&Style::new())?;
        }
        Ok(())
    }

    fn encode_chunks(&self, writer: &mut PaletteWriter<'_>, record: &Record) -> anyhow::Result<()> {
        let colors = record.key_values().get(COLORS_KEY.into());
        let inline = colors.and_then(|value| value.to_borrowed_str())
            .and_then(|json| serde_json::from_str::<Vec<ColorChange<'_>>>(json).ok())
            .and_then(|changes| {
                let message = record.args().to_string();
                let mut previous = 0;
                changes.iter().all(|change| {
                    let valid = change.offset >= previous && message.is_char_boundary(change.offset);
                    previous = change.offset;
                    valid
                }).then_some((message, changes))
            });
        let level = record.level() as usize - 1;
        for chunk in &self.chunks {
            match chunk {
                Chunk::Text(pattern) => pattern.encode(writer, record)?,
                Chunk::Message => {
                    if let Some((message, changes)) = &inline {
                        let mut start = 0;
                        for change in changes {
                            writer.write_all(message[start..change.offset].as_bytes())?;
                            let index = change.palette.as_deref().and_then(|name|
                                self.names.binary_search_by(|(key, _)|
                                    key.bytes().cmp(name.bytes().map(|byte| byte.to_ascii_lowercase())))
                                    .ok().and_then(|position| self.names[position].1));
                            self.set_color(writer, index)?;
                            start = change.offset;
                        }
                        writer.write_all(message[start..].as_bytes())?;
                    } else {
                        write!(writer, "{}", record.args())?;
                    }
                }
                Chunk::Color(indexes) => self.set_color(writer, indexes[level])?,
                Chunk::DynamicColor(pattern) => {
                    let mut key = Vec::new();
                    pattern.encode(&mut SimpleWriter(&mut key), record)?;
                    key.make_ascii_lowercase();
                    let index = self.names.binary_search_by(|(name, _)| name.as_bytes().cmp(&key))
                        .ok().and_then(|position| self.names[position].1);
                    self.set_color(writer, index)?;
                }
            }
        }
        Ok(())
    }
}

// Track native styles, including changes made by nested pattern formatters.
struct PaletteWriter<'a> {
    inner: &'a mut dyn Write,
    style: Style,
    capture: Option<&'a mut super::removals::RenderedLine>,
}

impl PaletteWriter<'_> {
    fn ansi(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        if let Some(line) = &mut self.capture {
            line.ansi(bytes);
            Ok(())
        } else {
            self.inner.write_all(bytes)
        }
    }
}

impl std::io::Write for PaletteWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        match &mut self.capture { Some(line) => line.write(bytes), None => self.inner.write(bytes) }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match &mut self.capture { Some(line) => line.flush(), None => self.inner.flush() }
    }
}

impl Write for PaletteWriter<'_> {
    fn set_style(&mut self, style: &Style) -> std::io::Result<()> {
        match &mut self.capture { Some(line) => line.set_style(style)?, None => self.inner.set_style(style)? }
        self.style = style.clone();
        Ok(())
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
    use crate::logging::test_resources as fixtures;
    use std::io;

    fn encoder(name: &str) -> PaletteEncoder {
        serde_yaml::from_value::<PaletteEncoderConfig>(fixtures::encoder(name)).unwrap().build().unwrap()
    }

    #[test]
    fn structured_fields_select_colors_or_reset_for_missing_names() {
        let config = serde_yaml::from_str::<PaletteEncoderConfig>(r##"
pattern: "{palette(team_{K(label)(all)})}{K(label)(ALL)} {palette(team_{K(faction)(unknown)})}{m}"
palette:
  team_all: "#112233"
  team_mason: "#778899"
  team_unknown: null
"##).unwrap();
        let mut encoder = config.build().unwrap();
        encoder.enabled = true;
        for (name, index) in &encoder.names {
            if let Some(index) = index {
                encoder.colors[*index].style.text(if name == "team_all" { Color::Green } else { Color::Red });
            }
        }
        for (faction, expected) in [(Some("MASON"), Some(Color::Red)), (Some("unknown"), None), (Some("absent"), None), (None, None)] {
            let fields: Vec<_> = faction.map(|value| ("faction", value)).into_iter().collect();
            let record = Record::builder().key_values(&fields).args(format_args!("message")).build();
            let mut output = StyledBuffer::default();
            encoder.encode(&mut output, &record).unwrap();
            assert_eq!(output.bytes, b"ALL message");
            assert_eq!(output.styles.iter().map(|(offset, style)| (*offset, style.text)).collect::<Vec<_>>(),
                [(0, Some(Color::Green)), (4, expected), (11, None)]);
        }
    }

    #[test]
    fn truecolor_background_does_not_pass_native_fallback_styles_to_ansi_writer() {
        let mut encoder = encoder("channels");
        encoder.enabled = true;
        encoder.truecolor = true;
        // Console configuration prepares native fallback styles even in RGB mode.
        for entry in &mut encoder.colors {
            entry.style.text(Color::Yellow).background(Color::Black).intense(false);
        }
        let message = crate::logging::inline::Message::new(&[
            crate::logging::inline::Segment::Palette("both"),
            crate::logging::inline::Segment::Text(format_args!("banner")),
            crate::logging::inline::Segment::Palette(""),
            crate::logging::inline::Segment::Text(format_args!("\r\nnext")),
        ]);
        let values = [(COLORS_KEY, message.colors.as_str())];
        let mut bytes = Vec::new();
        encoder.encode(&mut log4rs::encode::writer::ansi::AnsiWriter(&mut bytes),
            &Record::builder().args(format_args!("{}", message.text)).key_values(&values).build()).unwrap();
        assert_eq!(bytes, b"\x1b[0m\x1b[0;38;2;18;52;86;48;2;171;205;239mbanner\x1b[0m\r\nnext\x1b[0m");
    }

    #[test]
    fn foreground_and_background_entries_reset_omitted_channels() {
        use crate::logging::inline::{Message, Segment::{Palette, Text}};
        let mut encoder = encoder("channels");
        encoder.enabled = true;
        encoder.truecolor = true;
        let message = Message::new(&[
            Palette("both"), Text(format_args!("A")),
            Palette("background"), Text(format_args!("B")),
            Palette("foreground"), Text(format_args!("C")),
            Palette("object-foreground"), Text(format_args!("D")),
            Palette("normal"), Text(format_args!("E")),
        ]);
        let values = [(COLORS_KEY, message.colors.as_str())];
        let mut buffer = StyledBuffer::default();
        encoder.encode(&mut buffer, &Record::builder()
            .args(format_args!("{}", message.text)).key_values(&values).build()).unwrap();
        assert_eq!(buffer.bytes, b"\x1b[0;38;2;18;52;86;48;2;171;205;239mA\x1b[0;48;2;171;205;239mB\x1b[0;38;2;18;52;86mC\x1b[0;38;2;18;52;86mDE");
        assert_eq!(buffer.styles.last().unwrap().1, Style::new());

    }

    #[test]
    fn inherited_rgb_channels_do_not_reset_the_previous_color() {
        use crate::logging::inline::{Message, Segment::{Palette, Text}};
        let mut encoder = encoder("inherit");
        encoder.enabled = true;
        encoder.truecolor = true;
        let message = Message::new(&[
            Palette("both"), Text(format_args!("A")),
            Palette("keep"), Text(format_args!("B")),
            Palette("foreground"), Text(format_args!("C")),
            Palette("background"), Text(format_args!("D")),
            Palette("default-foreground"), Text(format_args!("E")),
            Palette("default-background"), Text(format_args!("F")),
            Palette("simple"), Text(format_args!("G")),
            Palette("normal"), Text(format_args!("H")),
        ]);
        let values = [(COLORS_KEY, message.colors.as_str())];
        let mut output = StyledBuffer::default();
        encoder.encode(&mut output, &Record::builder()
            .args(format_args!("{}", message.text)).key_values(&values).build()).unwrap();
        assert_eq!(output.bytes, b"\x1b[0;38;2;18;52;86;48;2;171;205;239mAB\x1b[38;2;254;220;186mC\x1b[48;2;101;67;33mD\x1b[39mE\x1b[49mF\x1b[49mGH");
        assert_eq!(output.styles.len(), 3, "inheriting RGB switches must not reset native style");
        assert_eq!(output.styles[1].0, output.bytes.len() - 1, "null resets before H");
    }

    #[test]
    fn inherited_native_channels_follow_palette_and_pattern_styles() {
        let mut encoder = encoder("inherit");
        encoder.enabled = true;
        for (name, index) in &encoder.names {
            let Some(index) = index else { continue };
            let style = &mut encoder.colors[*index].style;
            match name.as_str() {
                "both" => { style.text(Color::Blue).background(Color::Green).intense(true); },
                "foreground" => { style.text(Color::Red).intense(false); },
                "background" => { style.background(Color::Yellow); },
                _ => {},
            }
        }
        encoder.chunks = parse("{palette(both)}A{palette(keep)}B{palette(foreground)}C{palette(background)}D{palette(default-foreground)}E{palette(default-background)}F", &encoder.names.iter().cloned().collect()).unwrap().0;
        let mut output = StyledBuffer::default();
        encoder.encode(&mut output, &Record::builder().build()).unwrap();
        assert_eq!(output.bytes, b"ABCDEF");
        let styles: Vec<_> = output.styles.iter().map(|(_, style)| (style.text, style.background, style.intense)).collect();
        assert_eq!(styles, [
            (Some(Color::Blue), Some(Color::Green), Some(true)),
            (Some(Color::Red), Some(Color::Green), Some(false)),
            (Some(Color::Red), Some(Color::Yellow), Some(false)),
            (None, Some(Color::Yellow), None),
            (None, None, None), (None, None, None),
        ]);
        encoder.chunks = parse("{palette(both)}{h(x)}{palette(background)}{m}", &encoder.names.iter().cloned().collect()).unwrap().0;
        let mut output = StyledBuffer::default();
        encoder.encode(&mut output, &Record::builder().args(format_args!("Y")).build()).unwrap();
        assert_eq!(output.bytes, b"xY");
        let style = &output.styles[output.styles.len() - 2].1;
        assert_eq!(style.text, None, "inherit observes h's reset, not the earlier palette");
        assert_eq!(style.background, Some(Color::Yellow));
    }

    // Never invoked: registration must not depend on a log call being enabled or executed.
    #[allow(dead_code)]
    fn indexed_test_references() {
        sinfo!(palette("__TEST_PALETTE_PRESENT"), "fixture");
        swarn!(palette("__test_palette_missing"), "fixture");
        serror!(request = 42; palette("__TEST_PALETTE_MISSING"), "fixture");
        sdebug!(palette(r#"__test_palette_raw"#), "fixture");
        strace!(palette(""), "fixture", palette());
        sinfo!(palette("info"), "fixture");
        let dynamic = String::from("__test_palette_dynamic");
        sinfo!(palette(&dynamic), "fixture");
    }

    #[test]
    fn palette_loading_deduplicates_missing_names_for_console_and_file_encoders() {
        let path = std::env::temp_dir().join(format!("palette-index-{}.log", std::process::id()));
        for kind in ["console", "file"] {
            let mut yaml = fixtures::config("palette-index");
            yaml["appenders"]["test"]["kind"] = kind.into();
            if kind == "file" { yaml["appenders"]["test"]["path"] = path.to_string_lossy().as_ref().into(); }
            yaml["appenders"]["duplicate"] = yaml["appenders"]["test"].clone();
            let (config, missing) = super::super::config_with_warnings(yaml, "palette-index-test").unwrap();
            let fixture_names: Vec<_> = missing.iter().filter(|name| name.starts_with("__test_palette_")).cloned().collect();
            assert_eq!(fixture_names, ["__test_palette_missing", "__test_palette_raw"]);
            assert!(!missing.contains(""));
            drop(config);
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn palette_warnings_respect_encoder_kinds_defaults_and_per_encoder_names() {
        for kind in [None, Some("null"), Some("pattern"), Some("json")] {
            let mut yaml = fixtures::config("non-palette-index");
            let appender = &mut yaml["appenders"]["test"];
            match kind {
                None => { appender.as_mapping_mut().unwrap().remove("encoder"); },
                Some("null") => appender["encoder"] = serde_yaml::Value::Null,
                Some(kind) => appender["encoder"]["kind"] = kind.into(),
            }
            let (_, warnings) = super::super::config_with_warnings(yaml, "non-palette-test").unwrap();
            assert!(warnings.is_empty(), "{kind:?}");
        }
        let mut yaml = fixtures::config("palette-index");
        yaml["appenders"]["test"]["encoder"] = fixtures::encoder("defaults");
        yaml["appenders"]["test"]["encoder"]["kind"] = "palette".into();
        let (_, explicit) = super::super::config_with_warnings(yaml.clone(), "explicit-defaults").unwrap();
        assert!(!explicit.contains("info"));
        assert!(explicit.contains("__test_palette_present"));
        yaml["appenders"]["test"]["encoder"]["palette"] = serde_yaml::Value::Null;
        let (_, null) = super::super::config_with_warnings(yaml, "null-defaults").unwrap();
        assert_eq!(explicit, null);

        let mut yaml = fixtures::config("palette-index");
        yaml["appenders"]["other"] = yaml["appenders"]["test"].clone();
        yaml["appenders"]["other"]["encoder"]["palette"] = serde_yaml::Mapping::new().into();
        let (_, missing) = super::super::config_with_warnings(yaml, "per-encoder-names").unwrap();
        assert!(missing.contains("__test_palette_present"), "defined in one encoder but missing in another");
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
    fn removals_span_fields_and_colors_without_filtering_control_sequences() {
        for (enabled, truecolor) in [(false, false), (true, false), (true, true)] {
            let mut encoder = serde_yaml::from_str::<PaletteEncoderConfig>(r##"
pattern: "éDR{palette(red)}OP{m}{palette()}tail"
palette:
  red: "#112233"
inline-removals:
  - "DROPme"
  - "[0-9]+"
"##).unwrap().build().unwrap();
            encoder.enabled = enabled;
            encoder.truecolor = truecolor;
            encoder.colors[0].style.text(Color::Red);
            let record = Record::builder().args(format_args!("me42 kept")).build();
            let mut output = StyledBuffer::default();
            encoder.encode(&mut output, &record).unwrap();
            let expected = if enabled && truecolor { "é\x1b[0;38;2;17;34;51m kepttail" } else { "é kepttail" };
            assert_eq!(output.bytes, expected.as_bytes());
            if !enabled { assert!(output.styles.is_empty()); }
            if enabled && !truecolor {
                assert_eq!(output.styles[0].0, "é".len());
                assert_eq!(output.styles[0].1.text, Some(Color::Red));
            }
        }
    }

    #[test]
    fn names_are_resolved_at_startup_for_all_levels_and_repeated_switches() {
        let mut encoder = encoder("severity");
        assert_eq!(encoder.colors.len(), 6, "duplicate RGB values share an entry");
        for level in [Level::Error, Level::Warn, Level::Info, Level::Debug, Level::Trace] {
            let values = [("enclosing_function", "named"), ("request", "42")];
            let record = Record::builder().level(level).module_path(Some("UnchainedPlugin::messages"))
                .key_values(&values).args(format_args!("hello 世界 {{palette(text)}}")).build();
            let mut output = StyledBuffer::default();
            encoder.encode(&mut output, &record).unwrap();
            assert!(output.styles.is_empty(), "disabled colors must not change native styles");
            assert_eq!(String::from_utf8(output.bytes).unwrap(),
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
    fn empty_palette_with_plain_message_skips_console_configuration() {
        let mut encoder = encoder("empty");
        assert!(encoder.colors.is_empty());
        for target in [log4rs::append::console::Target::Stdout, log4rs::append::console::Target::Stderr] {
            encoder.configure(target);
            assert!(!encoder.enabled);
            assert!(!encoder.truecolor);
            let mut buffer = StyledBuffer::default();
            encoder.encode(&mut buffer, &Record::builder()
                .args(format_args!("sample 世界")).build()).unwrap();
            assert_eq!(buffer.bytes, "sample 世界".as_bytes());
            assert!(buffer.styles.is_empty());
        }
    }

    #[test]
    fn inline_switches_stream_palette_styles() {
        use std::sync::Mutex;
        struct Logger { encoder: PaletteEncoder, output: Mutex<StyledBuffer> }
        impl log::Log for Logger {
            fn enabled(&self, _: &log::Metadata) -> bool { true }
            fn log(&self, record: &Record) {
                self.encoder.encode(&mut *self.output.lock().unwrap(), record).unwrap();
            }
            fn flush(&self) {}
        }
        log::set_max_level(log::LevelFilter::Trace);
        for truecolor in [false, true] {
            let mut encoder = encoder("repeated-message");
            encoder.enabled = true;
            encoder.truecolor = truecolor;
            encoder.colors[0].style.text(Color::Magenta);
            let logger = Logger { encoder, output: Mutex::new(StyledBuffer::default()) };
            let reference = String::from("AcCeNt");
            sinfo!(logger: &logger, ("before {}", 7), palette(&reference), ("after {}", "世界"),
                palette("unknown"), "normal", palette("normal"), "end", palette());
            let output = logger.output.lock().unwrap();
            let message = if truecolor { "before 7\x1b[0;38;2;18;52;86mafter 世界normalend" }
                else { "before 7after 世界normalend" };
            assert_eq!(output.bytes, format!("{message}|{message}").as_bytes());
            assert_eq!(output.styles[0].0, 8);
            assert_eq!(output.styles[0].1.text, if truecolor { None } else { Some(Color::Magenta) });
            assert_eq!(output.styles[1].1, Style::new());
            assert_eq!(output.styles[1].0, message.find("normal").unwrap());
            assert_eq!(output.styles.last().unwrap().1, Style::new());
        }
    }

    #[test]
    fn pattern_and_inline_output_errors_reset_style() {
        use crate::logging::inline::{Message, Segment};
        let segments = [Segment::Text(format_args!("before")), Segment::Palette("warn"),
            Segment::Text(format_args!("after")), Segment::Palette("")];
        let message = Message::new(&segments);
        let values = [(COLORS_KEY, message.colors.as_str())];
        for (fixture, text, metadata) in [("accent", "hello", &[][..]), ("plain", message.text.as_str(), &values[..])] {
            let mut encoder = encoder(fixture);
            encoder.enabled = true;
            for truecolor in [false, true] {
                encoder.truecolor = truecolor;
                for limit in [0, 3, 6, 7] {
                    let mut output = StyledBuffer { fail_after: Some(limit), ..Default::default() };
                    let error = encoder.encode(&mut output, &Record::builder().key_values(&metadata)
                        .args(format_args!("{text}")).build()).unwrap_err();
                    assert!(error.to_string().contains("injected output failure"), "{fixture}, {truecolor}, {limit}");
                    assert_eq!(output.styles.last().unwrap().1, Style::new(), "{fixture}, {truecolor}, {limit}");
                }
            }
        }
    }

    #[test]
    fn invalid_inline_metadata_falls_back_to_plain_text() {
        let mut encoder = encoder("repeated-message");
        encoder.enabled = true;
        for colors in [
            "not json",
            r#"[{"utf8_offset":1,"palette":"accent"}]"#,
            r#"[{"utf8_offset":5,"palette":"accent"}]"#,
            r#"[{"utf8_offset":3,"palette":"accent"},{"utf8_offset":0,"palette":null}]"#,
        ] {
            let values = [(COLORS_KEY, colors)];
            let mut output = StyledBuffer::default();
            encoder.encode(&mut output, &Record::builder().key_values(&values)
                .args(format_args!("界x")).build()).unwrap();
            assert_eq!(output.bytes, "界x|界x".as_bytes(), "{colors}");
            assert_eq!(output.styles, [(9, Style::new())], "{colors}");
        }
    }

    #[test]
    fn escaped_palette_names_and_terminal_changes_replay_from_attributes() {
        let name = "a\"b\\c\n";
        let mut encoder = encoder("escaped-name");
        encoder.enabled = true;
        encoder.truecolor = true;
        let colors = serde_json::json!([
            {"utf8_offset": 0, "palette": name},
            {"utf8_offset": 3, "palette": "missing"},
            {"utf8_offset": 3, "palette": name},
            {"utf8_offset": 3, "palette": null}
        ]).to_string();
        let values = [(COLORS_KEY, colors.as_str())];
        let mut output = StyledBuffer::default();
        encoder.encode(&mut output, &Record::builder().key_values(&values)
            .args(format_args!("界")).build()).unwrap();
        assert_eq!(output.bytes, "\x1b[0;38;2;18;52;86m界\x1b[0;38;2;18;52;86m".as_bytes());
        assert_eq!(output.styles.len(), 5);
        assert_eq!(output.styles[1].1, Style::new());
        assert_eq!(output.styles[3].1, Style::new());
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
        let mut encoder = encoder("plain");
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
    }

}
