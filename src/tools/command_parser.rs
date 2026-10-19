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

    #[test]
    fn command_parser_preserves_raw_argument_spans() {
        let cases = [
            ("say hello", "say", vec!["hello"], "hello"),
            ("  😀", "😀", vec![], ""),
            (
                " \t😀 \"héllo 世界\"",
                "😀",
                vec!["héllo 世界"],
                "\"héllo 世界\"",
            ),
            ("\"😀\"", "😀", vec![], ""),
            ("\"😀\" trailing", "😀", vec!["trailing"], "trailing"),
            ("\"say stuff\" arg", "say stuff", vec!["arg"], "arg"),
            ("sa\\y hello", "say", vec!["hello"], "hello"),
            ("say\\ stuff '世界'", "say stuff", vec!["世界"], "'世界'"),
            ("\"sa\\\"y\" arg", "sa\"y", vec!["arg"], "arg"),
            ("'sa\\y' arg", "sa\\y", vec!["arg"], "arg"),
            ("say\\\nmore x", "saymore", vec!["x"], "x"),
            ("# skip\nsay hello", "say", vec!["hello"], "hello"),
            ("say # skip\nhello", "say", vec!["hello"], "# skip\nhello"),
            ("say\r arg", "say\r", vec!["arg"], "arg"),
            ("  '' x", "", vec!["x"], "x"),
            ("\\\n x", "", vec!["x"], "x"),
            ("say \"a\\\nb\"", "say", vec!["ab"], "\"a\\\nb\""),
            (
                " \t say  \"hello world\" --flag \n",
                "say",
                vec!["hello world", "--flag"],
                "\"hello world\" --flag",
            ),
        ];
        for (input, expected_name, expected_args, expected_raw) in cases {
            let (name, args, raw_args) = parse_command(input)
                .unwrap_or_else(|| panic!("valid command rejected: {:?}", input));
            assert_eq!(name, expected_name, "input: {:?}", input);
            assert_eq!(args, expected_args, "input: {:?}", input);
            assert_eq!(raw_args, expected_raw, "input: {:?}", input);
        }
    }

    #[test]
    fn command_parser_ignores_empty_and_malformed_input() {
        for input in [
            "",
            " \t\n",
            "# only a comment",
            "say \"unterminated",
            "say trailing\\",
            "'broken",
        ] {
            assert!(parse_command(input).is_none(), "input: {:?}", input);
        }
    }
}
