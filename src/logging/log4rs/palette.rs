use std::collections::BTreeMap;
use std::io::Write as _;
use log::{Level, Record};
use log4rs::config::{Deserialize, Deserializers};
use log4rs::encode::{writer::simple::SimpleWriter, Color, Encode, Style, Write};

use super::pattern::PatternEncoder;
use crate::logging::inline::{ColorChange, COLORS_KEY};

pub(crate) const DEFAULT_PATTERN: &str = "{palette(timestamp)}{d(%H:%M:%S%.3f)}{palette(trace)} | {palette({l})}{l:>5}{palette(trace)} | {palette(module)}{sm}::{fn}{palette(timestamp)} - {palette({l}_body)}{m}{n}";

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
}

#[derive(Debug)]
enum Chunk {
    Text(PatternEncoder),
    Message,
    Color([Option<usize>; 5]),
}

#[derive(Debug)]
struct Entry {
    channels: Channels,
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
    pub(super) fn default_names() -> impl Iterator<Item = String> {
        Self::default_palette().into_keys()
    }

    fn default_palette() -> BTreeMap<String, Option<PaletteColor>> {
        let mut palette: BTreeMap<_, _> = [("error", Some([242, 139, 130])), ("warn", Some([242, 204, 120])),
            ("info", Some([140, 180, 225])), ("debug", Some([128, 128, 128])),
            ("trace", Some([92, 92, 92])), ("error_body", Some([238, 238, 238])), ("warn_body", Some([238, 238, 238])),
            ("info_body", Some([238, 238, 238])), ("debug_body", Some([128, 128, 128])),
            ("trace_body", Some([92, 92, 92])), ("timestamp", Some([144, 144, 144])),
            ("module", Some([190, 157, 92])), ("message", Some([238, 238, 238]))]
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
        // Redirected and file output use the ordinary pattern encoder directly.
        if !self.enabled { return self.plain.encode(writer, record); }
        let result = self.encode_chunks(writer, record);
        let reset = writer.set_style(&Style::new());
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
                if self.truecolor { return writer.write_all(entry.ansi.as_bytes()); }
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
                writer.write_all(entry.ansi.as_bytes())?;
            } else {
                writer.set_style(&entry.style)?;
            }
        } else {
            writer.set_style(&Style::new())?;
        }
        Ok(())
    }

    fn encode_chunks(&self, writer: &mut dyn Write, record: &Record) -> anyhow::Result<()> {
        let mut writer = PaletteWriter { inner: writer, style: Style::new() };
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
                Chunk::Text(pattern) => pattern.encode(&mut writer, record)?,
                Chunk::Message => {
                    if let Some((message, changes)) = &inline {
                        let mut start = 0;
                        for change in changes {
                            writer.write_all(message[start..change.offset].as_bytes())?;
                            let index = change.palette.as_deref().and_then(|name|
                                self.names.binary_search_by(|(key, _)|
                                    key.bytes().cmp(name.bytes().map(|byte| byte.to_ascii_lowercase())))
                                    .ok().and_then(|position| self.names[position].1));
                            self.set_color(&mut writer, index)?;
                            start = change.offset;
                        }
                        writer.write_all(message[start..].as_bytes())?;
                    } else {
                        write!(writer, "{}", record.args())?;
                    }
                }
                Chunk::Color(indexes) => self.set_color(&mut writer, indexes[level])?,
            }
        }
        Ok(())
    }
}

// Track native styles, including changes made by nested pattern formatters.
struct PaletteWriter<'a> {
    inner: &'a mut dyn Write,
    style: Style,
}

impl std::io::Write for PaletteWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> { self.inner.write(bytes) }
    fn flush(&mut self) -> std::io::Result<()> { self.inner.flush() }
    fn write_all(&mut self, bytes: &[u8]) -> std::io::Result<()> { self.inner.write_all(bytes) }
    fn write_fmt(&mut self, args: std::fmt::Arguments<'_>) -> std::io::Result<()> { self.inner.write_fmt(args) }
}

impl Write for PaletteWriter<'_> {
    fn set_style(&mut self, style: &Style) -> std::io::Result<()> {
        self.inner.set_style(style)?;
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
        assert_eq!(encoder.colors.len(), 3, "string and foreground object share a style");
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

        let mut config = fixtures::encoder("channels");
        config["pattern"] = "{palette(both)}A{palette(background)}B{palette(foreground)}C{palette(object-foreground)}D{palette(normal)}E".into();
        let mut direct = serde_yaml::from_value::<PaletteEncoderConfig>(config).unwrap().build().unwrap();
        direct.enabled = true;
        direct.truecolor = true;
        let mut output = StyledBuffer::default();
        direct.encode(&mut output, &Record::builder().build()).unwrap();
        assert_eq!(output.bytes, buffer.bytes);
        assert_eq!(output.styles, buffer.styles);

        encoder.enabled = false;
        let mut plain = StyledBuffer::default();
        encoder.encode(&mut plain, &Record::builder()
            .args(format_args!("{}", message.text)).key_values(&values).build()).unwrap();
        assert_eq!(plain.bytes, b"ABCDE");
        assert!(plain.styles.is_empty());
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
    fn literal_palette_index_is_available_before_calls_execute() {
        let names: std::collections::BTreeSet<_> = inventory::iter::<crate::logging::inline::PaletteReference>
            .into_iter().map(|reference| reference.0).collect();
        assert!(names.contains("__TEST_PALETTE_PRESENT"));
        assert!(names.contains("__test_palette_missing"));
        assert!(names.contains("__TEST_PALETTE_MISSING"));
        assert!(names.contains("__test_palette_raw"));
        assert!(!names.contains("__test_palette_dynamic"));
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
    fn arbitrary_names_switch_legacy_styles() {
        let mut encoder = encoder("accent");
        encoder.enabled = true;
        encoder.colors[0].style.text(Color::Magenta).intense(true);
        let record = Record::builder().args(format_args!("hello")).build();
        let mut buffer = StyledBuffer::default();
        encoder.encode(&mut buffer, &record).unwrap();
        assert_eq!(buffer.bytes, b"helloend");
        assert_eq!(buffer.styles[0].1.text, Some(Color::Magenta));
        assert_eq!(buffer.styles[0].1.intense, Some(true));
        assert_eq!(buffer.styles[1], (5, Style::new()));
    }

    #[test]
    fn pattern_unused_colors_remain_available_to_inline_messages() {
        let mut table = BTreeMap::new();
        for i in 0..64 { table.insert(format!("color{i}"), Some(PaletteColor::Foreground(Channel::Rgb([i, i, i])))); }
        let encoder = PaletteEncoderConfig {
            pattern: Some("{palette(color63)}hello{palette()}".to_owned()), palette: Some(table),
        }.build().unwrap();
        assert_eq!(encoder.colors.len(), 64);
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
                let colors = json["attributes"][COLORS_KEY].as_str().unwrap();
                assert_eq!(serde_json::from_str::<serde_json::Value>(colors).unwrap(), serde_json::json!([
                    {"utf8_offset": 8, "palette": "AcCeNt"},
                    {"utf8_offset": 20, "palette": "unknown"},
                    {"utf8_offset": 26, "palette": "normal"},
                    {"utf8_offset": 29, "palette": null}
                ]));
                let values = [(COLORS_KEY, colors)];
                let mut replay = StyledBuffer::default();
                self.encoder.encode(&mut replay, &Record::builder().key_values(&values)
                    .args(format_args!("{}", json["message"].as_str().unwrap())).build()).unwrap();
                let output = self.output.lock().unwrap();
                assert_eq!(replay.bytes, output.bytes);
                assert_eq!(replay.styles, output.styles);
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
    fn inline_validation_preserves_lookup_and_formatting_counts() {
        use std::{cell::Cell, fmt};
        struct Message(Cell<usize>);
        impl fmt::Display for Message {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.set(self.0.get() + 1);
                f.write_str("界x")
            }
        }
        struct Metadata<'a> { colors: Option<&'a str>, lookups: Cell<usize> }
        impl log::kv::Source for Metadata<'_> {
            fn visit<'kvs>(&'kvs self, _: &mut dyn log::kv::VisitSource<'kvs>) -> Result<(), log::kv::Error> {
                panic!("encoder enumerated metadata")
            }
            fn get(&self, key: log::kv::Key<'_>) -> Option<log::kv::Value<'_>> {
                assert_eq!(key.as_str(), COLORS_KEY);
                self.lookups.set(self.lookups.get() + 1);
                self.colors.map(log::kv::Value::from)
            }
        }
        for (colors, parsed, valid, switches) in [
            (None, false, false, 0),
            (Some("not json"), false, false, 0),
            (Some("null"), false, false, 0),
            (Some("[]"), true, true, 0),
            (Some(r#"[{"utf8_offset":-1,"palette":"warn"}]"#), false, false, 0),
            (Some(r#"[{"utf8_offset":1,"palette":"warn"}]"#), true, false, 0),
            (Some(r#"[{"utf8_offset":5,"palette":"warn"}]"#), true, false, 0),
            (Some(r#"[{"utf8_offset":3,"palette":"warn"},{"utf8_offset":0,"palette":null}]"#), true, false, 0),
            (Some(r#"[{"utf8_offset":0,"palette":null},{"utf8_offset":3,"palette":null},{"utf8_offset":3,"palette":null},{"utf8_offset":4,"palette":null}]"#), true, true, 4),
        ] {
            for (pattern, chunks, expected) in [
                ("{m}|{message}", 2, "界x|界x"), ("界{message}|{m}", 2, "界界x|界x"), ("prefix", 0, "prefix"),
            ] {
                let mut config = fixtures::encoder("plain");
                config["pattern"] = pattern.into();
                let mut encoder = serde_yaml::from_value::<PaletteEncoderConfig>(config).unwrap().build().unwrap();
                encoder.enabled = true;
                for truecolor in [false, true] {
                    encoder.truecolor = truecolor;
                    let message = Message(Cell::new(0));
                    let metadata = Metadata { colors, lookups: Cell::new(0) };
                    let mut output = StyledBuffer::default();
                    encoder.encode(&mut output, &Record::builder().key_values(&metadata)
                        .args(format_args!("{message}")).build()).unwrap();
                    assert_eq!(output.bytes, expected.as_bytes(), "{colors:?}, {pattern}, truecolor={truecolor}");
                    assert_eq!(metadata.lookups.get(), 1);
                    assert_eq!(message.0.get(), usize::from(parsed) + if valid { 0 } else { chunks }, "{colors:?}, {pattern}");
                    assert_eq!(output.styles.len(), switches * chunks + 1, "{colors:?}, {pattern}");
                    assert!(output.styles.iter().all(|(_, style)| *style == Style::new()));
                    assert_eq!(output.styles.last().unwrap().0, expected.len());
                }
            }
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
        let mut encoder = encoder("plain");
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
    fn shipped_and_builtin_defaults_preserve_colors_slots_and_message_codes() {
        let mut yaml: serde_yaml::Value = serde_yaml::from_str(super::super::EMBEDDED_LOG4RS_YAML).unwrap();
        let block = &mut yaml["appenders"]["console"]["encoder"];
        block.as_mapping_mut().unwrap().remove(serde_yaml::Value::String("kind".into()));
        let shipped = serde_yaml::from_value::<PaletteEncoderConfig>(block.clone()).unwrap().build().unwrap();
        let expected = [[242, 139, 130], [242, 204, 120], [140, 180, 225], [128, 128, 128], [92, 92, 92],
            [238, 238, 238], [190, 157, 92], [208, 145, 40], [144, 144, 144]];
        for mut encoder in [shipped, encoder("defaults")] {
            assert_eq!(encoder.colors.iter().map(|entry| entry.channels.foreground.unwrap()).collect::<Vec<_>>(), expected.map(Channel::Rgb));
            assert!(encoder.colors[..7].iter().all(|entry| entry.channels.background.is_none()));
            assert_eq!(encoder.colors[7].channels.background, Some(Channel::Rgb([47, 47, 47])));
            for name in ["title", "subtitle"] {
                assert_eq!(encoder.names.iter().find(|(key, _)| key == name).unwrap().1, Some(7));
            }
            encoder.enabled = true;
            encoder.truecolor = true;
            for (level, rgb) in [(Level::Error, "242;139;130"), (Level::Warn, "242;204;120"),
                (Level::Info, "140;180;225"), (Level::Debug, "128;128;128"), (Level::Trace, "92;92;92")] {
                let record = Record::builder().level(level).module_path(Some("plugin::module"))
                    .args(format_args!("before \x1b[31mred\x1b[0m after")).build();
                let mut bytes = Vec::new();
                encoder.encode(&mut log4rs::encode::writer::ansi::AnsiWriter(&mut bytes), &record).unwrap();
                let text = String::from_utf8(bytes).unwrap();
                let styled = text.replace("\x1b[0m", "");
                let severity = format!("\x1b[0;38;2;92;92;92m | \x1b[0;38;2;{rgb}m{level:>5}\x1b[0;38;2;92;92;92m | ");
                assert!(styled.contains(&severity), "{text:?}");
                assert!(styled.starts_with("\x1b[0;38;2;144;144;144m"));
                let body_rgb = if level <= Level::Info { "238;238;238" } else { rgb };
                assert!(styled.contains(&format!("\x1b[0;38;2;190;157;92mp::module::???\x1b[0;38;2;144;144;144m - \x1b[0;38;2;{body_rgb}mbefore")));
                assert!(text.contains("before \x1b[31mred\x1b[0m after"));
                assert!(text.ends_with("\x1b[0m"));
            }
        }
    }
}
