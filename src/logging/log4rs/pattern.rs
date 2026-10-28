use log::{kv::{Source, Value}, Record};
use log4rs::config::{Deserialize, Deserializers};
use log4rs::encode::{pattern::PatternEncoder as BuiltinPatternEncoder, Encode, Write};

const SHORT_MODULE_KEY: &str = "__unchained_short_module";
const FUNCTION_KEY: &str = "enclosing_function";

#[derive(Debug)]
pub struct PatternEncoder {
    inner: BuiltinPatternEncoder,
    extended: bool,
}

impl PatternEncoder {
    pub fn new(pattern: &str) -> Self {
        // Alias only formatter names, preserving escaped literals and nested patterns.
        let mut rest = pattern;
        let mut expanded = String::new();
        let mut extended = false;
        while !rest.is_empty() {
            let alias = [("sm", SHORT_MODULE_KEY), ("fn", FUNCTION_KEY)].into_iter()
                .find(|(name, _)| rest.strip_prefix('{').and_then(|rest| rest.strip_prefix(name))
                    .is_some_and(|rest| rest.starts_with('}') || rest.starts_with(':')));
            if let Some((_, key)) = alias {
                expanded.push_str(&format!("{{K({key})"));
                rest = &rest[3..];
                extended = true;
            } else {
                let ch = rest.chars().next().unwrap();
                expanded.push(ch);
                rest = &rest[ch.len_utf8()..];
                if ch == '\\' || (ch == '{' && rest.starts_with('{')) {
                    if let Some(escaped) = rest.chars().next() {
                        expanded.push(escaped);
                        rest = &rest[escaped.len_utf8()..];
                    }
                }
            }
        }
        Self { inner: BuiltinPatternEncoder::new(&expanded), extended }
    }
}

impl Encode for PatternEncoder {
    fn encode(&self, writer: &mut dyn Write, record: &Record<'_>) -> anyhow::Result<()> {
        if !self.extended { return self.inner.encode(writer, record); }
        let module = ShortModule(record.module_path().unwrap_or("???"));
        let function = record.key_values().get(FUNCTION_KEY.into()).unwrap_or(Value::from("???"));
        let values = [(SHORT_MODULE_KEY, Value::from_display(&module)), (FUNCTION_KEY, function)];
        let sources: [&dyn Source; 2] = [&values, record.key_values()];
        let extended = Record::builder()
            .metadata(record.metadata().clone())
            .args(*record.args())
            .module_path(record.module_path())
            .file(record.file())
            .line(record.line())
            .key_values(&sources)
            .build();
        self.inner.encode(writer, &extended)
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatternEncoderConfig {
    pattern: Option<String>,
}

pub struct PatternEncoderDeserializer;

impl Deserialize for PatternEncoderDeserializer {
    type Trait = dyn Encode;
    type Config = PatternEncoderConfig;

    fn deserialize(&self, config: PatternEncoderConfig, _: &Deserializers)
        -> anyhow::Result<Box<dyn Encode>> {
        Ok(Box::new(PatternEncoder::new(
            config.pattern.as_deref().unwrap_or("{d} {l} {t} - {m}{n}"),
        )))
    }
}

struct ShortModule<'a>(&'a str);

impl std::fmt::Display for ShortModule<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Retain the last 32 Unicode characters without allocating a label.
        let mut tail = ['\0'; 32];
        let mut next = 0;
        let mut length = 0;
        let mut push_char = |ch| {
            tail[next] = ch;
            next = (next + 1) % tail.len();
            length = (length + 1).min(tail.len());
        };
        let mut segments = self.0.split("::").peekable();
        while let Some(segment) = segments.next() {
            if segments.peek().is_some() {
                for word in segment.split('_') {
                    let mut chars = word.chars().peekable();
                    let mut previous_uppercase = false;
                    let mut first = true;
                    while let Some(ch) = chars.next() {
                        let uppercase = ch.is_uppercase();
                        // Start camel-case words and acronym-to-word transitions.
                        if first || (uppercase && (!previous_uppercase
                            || chars.peek().is_some_and(|next| next.is_lowercase()))) {
                            push_char(ch);
                        }
                        first = false;
                        previous_uppercase = uppercase;
                    }
                }
                push_char(':');
                push_char(':');
            } else {
                for ch in segment.chars() { push_char(ch); }
            }
        }
        let start = if length == tail.len() { next } else { 0 };
        for offset in 0..length {
            write!(f, "{}", tail[(start + offset) % tail.len()])?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use log4rs::encode::writer::simple::SimpleWriter;

    #[test]
    fn short_module_field_composes_without_changing_standard_fields_or_records() {
        let values = [("request", "42")];
        let record = Record::builder().module_path(Some("UnchainedPlugin::foo_bar::messages"))
            .target("filter::target").file(Some("source.rs")).line(Some(17))
            .key_values(&values).args(format_args!("literal {{sm}} message")).build();
        let mut output = Vec::new();
        PatternEncoder::new("{sm}|{M}|{t}|{f}:{L}|{K(request)}|{m}")
            .encode(&mut SimpleWriter(&mut output), &record).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(),
            "UP::fb::messages|UnchainedPlugin::foo_bar::messages|filter::target|source.rs:17|42|literal {sm} message");
        assert_eq!(record.module_path(), Some("UnchainedPlugin::foo_bar::messages"));
        assert!(record.key_values().get(SHORT_MODULE_KEY.into()).is_none());
    }

    #[test]
    fn short_module_boundaries_and_fixed_default_width_leave_custom_widths_independent() {
        use super::super::palette::PaletteEncoderConfig;

        let newline = if cfg!(windows) { "\r\n" } else { "\n" };
        let yaml: serde_yaml::Value = serde_yaml::from_str(super::super::EMBEDDED_LOG4RS_YAML).unwrap();
        let pattern = yaml["appenders"]["console"]["encoder"]["pattern"].as_str().unwrap();
        let config: PaletteEncoderConfig = serde_yaml::from_str("{}").unwrap();
        let encoder = config.build().unwrap();
        let config: PaletteEncoderConfig = serde_yaml::from_str(&format!("pattern: '{pattern}'")).unwrap();
        let configured_encoder = config.build().unwrap();
        for character in ['a', 'é', '界'] {
            for length in [1, 23, 24, 25, 31, 32, 33, 40] {
                let module: String = std::iter::once('Ω')
                    .chain(std::iter::repeat_n(character, length - 1)).collect();
                let expected: String = module.chars().rev().take(32)
                    .collect::<Vec<_>>().into_iter().rev().collect();
                let record = Record::builder().module_path(Some(&module))
                    .args(format_args!("message")).build();
                let mut custom = Vec::new();
                PatternEncoder::new("[{sm}]|[{sm:<24}]")
                    .encode(&mut SimpleWriter(&mut custom), &record).unwrap();
                assert_eq!(String::from_utf8(custom).unwrap(),
                    format!("[{expected}]|[{expected:<24}]"), "{module}");
                let mut console = Vec::new();
                encoder.encode(&mut SimpleWriter(&mut console), &record).unwrap();
                let suffix: String = module.chars().rev().take(24)
                    .collect::<Vec<_>>().into_iter().rev().collect();
                let label = format!("{suffix:<24}");
                let expected_body = format!(" [{label}] message{newline}");
                assert!(String::from_utf8(console).unwrap()
                    .ends_with(&expected_body), "{module}");
                let mut configured = Vec::new();
                configured_encoder.encode(&mut SimpleWriter(&mut configured), &record).unwrap();
                assert!(String::from_utf8(configured).unwrap().ends_with(&expected_body), "{module}");
            }
        }
    }

    #[test]
    fn aliases_respect_escaping_nesting_format_specs_and_missing_values() {
        let record = Record::builder().module_path(Some("UnchainedPlugin::messages")).build();
        for (pattern, expected) in [
            ("{{sm}}", "{sm}"),
            (r"\{sm\}", "{sm}"),
            ("{{{sm}}}", "{UP::messages}"),
            ("{sm}|{sm}", "UP::messages|UP::messages"),
            ("{({sm}):>15}", "   UP::messages"),
            ("{sm:>15}", "   UP::messages"),
            ("{sm:<24}", "UP::messages            "),
            ("{sm:-.8}", "messages"),
            ("{{fn}}", "{fn}"), (r"\{fn\}", "{fn}"), ("{{{fn}}}", "{???}"),
            ("{fn}|{fn}", "???|???"), ("{fn:>5}", "  ???"),
            ("{({fn}):>5}", "  ???"), ("{fn:.2}", "??"),
        ] {
            let mut output = Vec::new();
            PatternEncoder::new(pattern).encode(&mut SimpleWriter(&mut output), &record).unwrap();
            assert_eq!(String::from_utf8(output).unwrap(), expected, "{pattern}");
        }
    }
}
