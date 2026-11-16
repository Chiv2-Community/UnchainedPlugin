#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Header<'a> {
    System { channel: &'a str, body: &'a str },
    Player,
}

fn channel_prefix(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix('<')?;
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let body = rest[digits..].strip_prefix(">:")?;
    Some((&rest[..digits], body.trim_start_matches([' ', '\t'])))
}

pub(crate) fn parse(line: &str) -> Option<Header<'_>> {
    if let Some((channel, body)) = channel_prefix(line) {
        return Some(Header::System { channel, body });
    }
    let first_line = line.split(['\r', '\n']).next().unwrap_or_default();
    for (index, _) in first_line.match_indices('<') {
        if index > 0
            && matches!(first_line.as_bytes()[index - 1], b' ' | b'\t')
            && channel_prefix(&line[index..]).is_some()
        {
            return Some(Header::Player);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_headers_preserve_digits_and_body() {
        for (line, channel, body) in [
            ("<0>: Hello", "0", "Hello"),
            ("<1>:\t \r\nHello", "1", "\r\nHello"),
            ("<6>:", "6", ""),
            ("<6>:Welcome", "6", "Welcome"),
            ("<0>: 雪\nPlayer <1>: hello", "0", "雪\nPlayer <1>: hello"),
            ("<4294967296>: literal", "4294967296", "literal"),
            ("<000>: text", "000", "text"),
        ] {
            assert_eq!(parse(line), Some(Header::System { channel, body }));
        }
    }

    #[test]
    fn player_headers_are_recognized_before_parsing_channel_values() {
        for line in [
            "Player Name <0>: [Console] Hello",
            "Player <1>:",
            "Player <4294967296>: !vote kick\r\nreason",
            "雪\t<0>: text",
            " <0>: text",
            "Player <x>: text <1>: hello",
            "<x>: text <1>: hello",
        ] {
            assert_eq!(parse(line), Some(Header::Player), "{line:?}");
        }
    }

    #[test]
    fn malformed_headers_remain_literal() {
        for line in [
            "",
            "Welcome!",
            "Welcome\nPlayer <0>: text",
            "Welcome\rPlayer <0>: text",
            "<x>: text",
            "<>: text",
            "<+1>: text",
            "<１>: text",
            "<1> text",
            "Player<0>: text",
            "Player\u{a0}<0>: text",
            "Player <0 >: text",
        ] {
            assert_eq!(parse(line), None, "{line:?}");
        }
    }
}
