use log::{kv::{Source, Value}, Record};
use log4rs::config::{Deserialize, Deserializers};
use log4rs::encode::{pattern::PatternEncoder as BuiltinPatternEncoder, Encode, Write};

const SHORT_MODULE_KEY: &str = "__unchained_short_module";
const FUNCTION_KEY: &str = "enclosing_function";
const DISPLAY_FUNCTION_KEY: &str = "__unchained_function";

#[derive(Debug)]
pub struct PatternEncoder {
    inner: BuiltinPatternEncoder,
    extended: bool,
    inline_removals: Option<regex::Regex>,
}

impl PatternEncoder {
    pub fn new(pattern: &str) -> Self {
        // Alias only formatter names, preserving escaped literals and nested patterns.
        let mut rest = pattern;
        let mut expanded = String::new();
        let mut extended = false;
        while !rest.is_empty() {
            let alias = [("sm", SHORT_MODULE_KEY), ("fn", DISPLAY_FUNCTION_KEY)].into_iter()
                .find(|(name, _)| rest.strip_prefix('{').and_then(|rest| rest.strip_prefix(name))
                    .is_some_and(|rest| rest.starts_with('}') || rest.starts_with(':')));
            if let Some((_, key)) = alias {
                expanded.push_str("{K(");
                expanded.push_str(key);
                expanded.push(')');
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
        Self { inner: BuiltinPatternEncoder::new(&expanded), extended, inline_removals: None }
    }
}

impl Encode for PatternEncoder {
    fn encode(&self, writer: &mut dyn Write, record: &Record<'_>) -> anyhow::Result<()> {
        if let Some(regex) = &self.inline_removals {
            let mut line = super::removals::RenderedLine::default();
            self.encode_pattern(&mut line, record)?;
            return line.emit(writer, regex);
        }
        self.encode_pattern(writer, record)
    }
}

impl PatternEncoder {
    fn encode_pattern(&self, writer: &mut dyn Write, record: &Record<'_>) -> anyhow::Result<()> {
        if !self.extended { return self.inner.encode(writer, record); }
        let module = ShortModule(record.module_path().unwrap_or("???"));
        let function = record.key_values().get(FUNCTION_KEY.into())
            .map(|value| value.to_string()).unwrap_or_else(|| "???".into());
        let function = function.strip_suffix("_detour_fkt").unwrap_or(&function);
        let values = [(SHORT_MODULE_KEY, Value::from_display(&module)),
            (DISPLAY_FUNCTION_KEY, Value::from(function))];
        let sources: [&dyn Source; 2] = [&values, record.key_values()];
        let extended = record.to_builder().key_values(&sources).build();
        self.inner.encode(writer, &extended)
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatternEncoderConfig {
    pattern: Option<String>,
    #[serde(default, rename = "inline-removals")]
    inline_removals: Vec<String>,
}

pub struct PatternEncoderDeserializer;

impl Deserialize for PatternEncoderDeserializer {
    type Trait = dyn Encode;
    type Config = PatternEncoderConfig;

    fn deserialize(&self, config: PatternEncoderConfig, _: &Deserializers)
        -> anyhow::Result<Box<dyn Encode>> {
        let mut encoder = PatternEncoder::new(config.pattern.as_deref().unwrap_or("{d} {l} {t} - {m}{n}"));
        encoder.inline_removals = super::removals::compile(&config.inline_removals)?;
        Ok(Box::new(encoder))
    }
}

struct ShortModule<'a>(&'a str);

impl std::fmt::Display for ShortModule<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut label = String::new();
        if let Some((prefix, last)) = self.0.rsplit_once("::") {
            for segment in prefix.split("::") {
                for word in segment.split('_') {
                    let mut chars = word.chars().enumerate().peekable();
                    let mut previous_uppercase = false;
                    while let Some((index, ch)) = chars.next() {
                        let uppercase = ch.is_uppercase();
                        // Start camel-case words and acronym-to-word transitions.
                        if index == 0 || (uppercase && (!previous_uppercase
                            || chars.peek().is_some_and(|(_, next)| next.is_lowercase()))) {
                            label.push(ch);
                        }
                        previous_uppercase = uppercase;
                    }
                }
                label.push_str("::");
            }
            label.push_str(last);
        } else {
            label.push_str(self.0);
        }
        let start = label.char_indices().rev().nth(31).map_or(0, |(index, _)| index);
        f.write_str(&label[start..])
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
    fn short_module_truncates_unicode_from_the_left() {
        let module = format!("Ω{}", "界".repeat(32));
        let record = Record::builder().module_path(Some(&module)).build();
        let mut output = Vec::new();
        PatternEncoder::new("{sm}").encode(&mut SimpleWriter(&mut output), &record).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), "界".repeat(32));
    }

    #[test_case::test_case("Tick_detour_fkt", "Tick"; "detour_suffix")]
    #[test_case::test_case("long_function_name_that_exceeds_thirty_two_characters_detour_fkt", "at_exceeds_thirty_two_characters"; "left_truncation")]
    #[test_case::test_case("keep_detour_fkt_inside", "keep_detour_fkt_inside"; "interior_suffix")]
    fn function_display_preserves_raw_metadata(name: &str, display: &str) {
        let values = [(FUNCTION_KEY, name)];
        let record = Record::builder().key_values(&values).build();
        let mut output = Vec::new();
        PatternEncoder::new("{fn:<-32.32}|{K(enclosing_function)}")
            .encode(&mut SimpleWriter(&mut output), &record).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), format!("{display:<32}|{name}"));
    }

    #[test_case::test_case("{{sm}}", "{sm}"; "module_brace_escape")]
    #[test_case::test_case(r"\{sm\}", "{sm}"; "module_backslash_escape")]
    #[test_case::test_case("{{{sm}}}", "{UP::messages}"; "module_surrounding_braces")]
    #[test_case::test_case("{sm}|{sm}", "UP::messages|UP::messages"; "repeated_module")]
    #[test_case::test_case("{({sm}):>15}", "   UP::messages"; "nested_module_alignment")]
    #[test_case::test_case("{sm:>15}", "   UP::messages"; "module_right_alignment")]
    #[test_case::test_case("{sm:<24}", "UP::messages            "; "module_left_alignment")]
    #[test_case::test_case("{sm:-.8}", "messages"; "module_left_truncation")]
    #[test_case::test_case("{{fn}}", "{fn}"; "function_brace_escape")]
    #[test_case::test_case(r"\{fn\}", "{fn}"; "function_backslash_escape")]
    #[test_case::test_case("{{{fn}}}", "{???}"; "missing_function_surrounding_braces")]
    #[test_case::test_case("{fn}|{fn}", "???|???"; "repeated_missing_function")]
    #[test_case::test_case("{fn:>5}", "  ???"; "missing_function_alignment")]
    #[test_case::test_case("{({fn}):>5}", "  ???"; "nested_missing_function_alignment")]
    #[test_case::test_case("{fn:.2}", "??"; "missing_function_truncation")]
    fn aliases_respect_pattern_syntax(pattern: &str, expected: &str) {
        let record = Record::builder().module_path(Some("UnchainedPlugin::messages")).build();
        let mut output = Vec::new();
        PatternEncoder::new(pattern).encode(&mut SimpleWriter(&mut output), &record).unwrap();
        assert_eq!(String::from_utf8(output).unwrap(), expected);
    }
}
