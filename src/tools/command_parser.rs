pub fn parse_command(content: &str) -> Option<(String, Vec<String>, String)> {
    let mut parts = shlex::split(content)?.into_iter();
    let name = parts.next()?;
    let args = parts.collect();
    let mut raw_args = "";
    let mut started = false;
    let mut comment = false;
    let mut quote = None;
    let mut escaped = false;

    for (offset, ch) in content.char_indices() {
        if !started {
            if comment {
                if ch == '\n' {
                    comment = false;
                }
                continue;
            }
            match ch {
                ' ' | '\t' | '\n' => continue,
                '#' => {
                    comment = true;
                    continue;
                }
                _ => started = true,
            }
        }
        if quote == Some('\'') {
            if ch == '\'' {
                quote = None;
            }
            continue;
        }
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => quote = if quote.is_some() { None } else { Some('"') },
            '\'' if quote.is_none() => quote = Some('\''),
            ' ' | '\t' | '\n' if quote.is_none() => {
                // Keep everything after the command name, trimming surrounding whitespace.
                raw_args = content[offset..].trim();
                break;
            }
            _ => {}
        }
    }
    Some((name, args, raw_args.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case::test_case("say hello", "say", &["hello"], "hello"; "plain")]
    #[test_case::test_case("  😀", "😀", &[], ""; "unicode_name_without_args")]
    #[test_case::test_case(" \t😀 \"héllo 世界\"", "😀", &["héllo 世界"], "\"héllo 世界\""; "unicode_quoted_argument")]
    #[test_case::test_case("\"😀\"", "😀", &[], ""; "quoted_name_without_args")]
    #[test_case::test_case("\"😀\" trailing", "😀", &["trailing"], "trailing"; "quoted_name_with_args")]
    #[test_case::test_case("\"say stuff\" arg", "say stuff", &["arg"], "arg"; "spaces_in_name")]
    #[test_case::test_case("sa\\y hello", "say", &["hello"], "hello"; "escaped_name_character")]
    #[test_case::test_case("say\\ stuff '世界'", "say stuff", &["世界"], "'世界'"; "escaped_name_space")]
    #[test_case::test_case("\"sa\\\"y\" arg", "sa\"y", &["arg"], "arg"; "escaped_quote")]
    #[test_case::test_case("'sa\\y' arg", "sa\\y", &["arg"], "arg"; "single_quoted_backslash")]
    #[test_case::test_case("say\\\nmore x", "saymore", &["x"], "x"; "continued_name")]
    #[test_case::test_case("# skip\nsay hello", "say", &["hello"], "hello"; "leading_comment")]
    #[test_case::test_case("say # skip\nhello", "say", &["hello"], "# skip\nhello"; "argument_comment")]
    #[test_case::test_case("say\r arg", "say\r", &["arg"], "arg"; "carriage_return_in_name")]
    #[test_case::test_case("  '' x", "", &["x"], "x"; "empty_quoted_name")]
    #[test_case::test_case("\\\n x", "", &["x"], "x"; "empty_continued_name")]
    #[test_case::test_case("say \"a\\\nb\"", "say", &["ab"], "\"a\\\nb\""; "continued_argument")]
    #[test_case::test_case(" \t say  \"hello world\" --flag \n", "say", &["hello world", "--flag"], "\"hello world\" --flag"; "surrounding_whitespace")]
    fn command_parser_preserves_raw_argument_spans(input: &str, name: &str, args: &[&str], raw: &str) {
        let expected = (name.to_owned(), args.iter().map(|arg| (*arg).to_owned()).collect(), raw.to_owned());
        assert_eq!(parse_command(input), Some(expected));
    }

    #[test_case::test_case(""; "empty")]
    #[test_case::test_case(" \t\n"; "whitespace")]
    #[test_case::test_case("# only a comment"; "comment")]
    #[test_case::test_case("say \"unterminated"; "unclosed_double_quote")]
    #[test_case::test_case("say trailing\\"; "trailing_escape")]
    #[test_case::test_case("'broken"; "unclosed_single_quote")]
    fn command_parser_ignores_empty_and_malformed_input(input: &str) {
        assert!(parse_command(input).is_none());
    }
}
