const TITLE: &str = "\
\
▄████████    ▄█    █▄     ▄█   ▄█    █▄     ▄████████  ▄█          ▄████████ ▄██   ▄         ▄█   ▄█ \r\n\
███    ███   ███    ███   ███  ███    ███   ███    ███ ███         ███    ███ ███   ██▄      ███  ███ \r\n\
███    █▀    ███    ███   ███▌ ███    ███   ███    ███ ███         ███    ███ ███▄▄▄███      ███▌ ███▌\r\n\
███         ▄███▄▄▄▄███▄▄ ███▌ ███    ███   ███    ███ ███        ▄███▄▄▄▄██▀ ▀▀▀▀▀▀███      ███▌ ███▌\r\n\
███        ▀▀███▀▀▀▀███▀  ███▌ ███    ███ ▀███████████ ███       ▀▀███▀▀▀▀▀   ▄██   ███      ███▌ ███▌\r\n\
███    █▄    ███    ███   ███  ███    ███   ███    ███ ███       ▀███████████ ███   ███      ███  ███ \r\n\
███    ███   ███    ███   ███  ███    ███   ███    ███ ███▌    ▄   ███    ███ ███   ███      ███  ███ \r\n\
████████▀    ███    █▀    █▀    ▀██████▀    ███    █▀  █████▄▄██   ███    ███  ▀█████▀       █▀   █▀  \r\n\
                                                       ▀           ███    ███                         \r\n\
";

const SUBTITLE: &str = "███    █▄  ███▄▄▄▄    ▄████████    ▄█    █▄       ▄████████  ▄█  ███▄▄▄▄      ▄████████ ████████▄     \r\n\
███    ███ ███▀▀▀██▄ ███    ███   ███    ███     ███    ███ ███  ███▀▀▀██▄   ███    ███ ███   ▀███    \r\n\
███    ███ ███   ███ ███    █▀    ███    ███     ███    ███ ███▌ ███   ███   ███    █▀  ███    ███    \r\n\
███    ███ ███   ███ ███         ▄███▄▄▄▄███▄▄   ███    ███ ███▌ ███   ███  ▄███▄▄▄     ███    ███    \r\n\
███    ███ ███   ███ ███        ▀▀███▀▀▀▀███▀  ▀███████████ ███▌ ███   ███ ▀▀███▀▀▀     ███    ███    \r\n\
███    ███ ███   ███ ███    █▄    ███    ███     ███    ███ ███  ███   ███   ███    █▄  ███    ███    \r\n\
███    ███ ███   ███ ███    ███   ███    ███     ███    ███ ███  ███   ███   ███    ███ ███   ▄███    \r\n\
████████▀   ▀█   █▀  ████████▀    ███    █▀      ███    █▀  █▀    ▀█   █▀    ██████████ ████████▀     \r\n\
";

pub(crate) fn print_banner() {
    log_banner(log::logger());
}

fn padded_lines(text: &str) -> String {
    text.lines().map(|line| format!(" {line}\r\n")).collect()
}

fn log_banner(logger: &dyn log::Log) {
    let width = TITLE.lines().chain(SUBTITLE.lines()).map(|line| line.chars().count()).max().unwrap() + 1;
    let blank = " ".repeat(width);
    let title = padded_lines(TITLE);
    let subtitle = padded_lines(SUBTITLE);
    // Reset before the final newline so scrolling cannot paint the next log row gray.
    sinfo!(logger: logger, ("\r\n"), palette("title"), "{blank}\r\n{title}",
        palette("subtitle"), ("{}", subtitle.trim_end_matches("\r\n")), palette(), "\r\n");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::logging::inline::{ColorChange, COLORS_KEY};

    struct CheckBanner;

    impl log::Log for CheckBanner {
        fn enabled(&self, _: &log::Metadata<'_>) -> bool { true }
        fn log(&self, record: &log::Record<'_>) {
            let text = record.args().to_string();
            let colors = record.key_values().get(COLORS_KEY.into()).unwrap().to_string();
            let changes: Vec<ColorChange<'_>> = serde_json::from_str(&colors).unwrap();
            let lines: Vec<_> = text.lines().collect();
            assert_eq!(lines[0], "");
            assert!(lines[1].chars().all(|ch| ch == ' '));
            assert_eq!(lines[1].len(), TITLE.lines().chain(SUBTITLE.lines())
                .map(|line| line.chars().count()).max().unwrap() + 1);
            let artwork: Vec<_> = TITLE.lines().chain(SUBTITLE.lines()).collect();
            assert_eq!(lines.len(), artwork.len() + 2);
            for (line, original) in lines[2..].iter().zip(artwork) {
                assert_eq!(*line, format!(" {original}"));
            }
            assert_eq!(changes.len(), 3);
            assert_eq!(changes[0].palette.as_deref(), Some("title"));
            assert_eq!(changes[0].offset, 2, "top padding must be colored");
            assert_eq!(changes[1].palette.as_deref(), Some("subtitle"));
            assert!(text[changes[1].offset..].starts_with(" ███"));
            assert!(changes[2].palette.is_none());
            assert_eq!(&text[changes[2].offset..], "\r\n", "reset before advancing to the next row");
            assert!(!text.contains('\x1b'), "plain output must not contain ANSI escapes");
        }
        fn flush(&self) {}
    }

    #[test]
    fn banner_has_colored_top_padding_indented_lines_and_resets_before_newline() {
        log::set_max_level(log::LevelFilter::Trace);
        log_banner(&CheckBanner);
    }
}
