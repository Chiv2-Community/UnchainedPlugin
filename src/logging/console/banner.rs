const PADDING: &str = "                                                                                                       ";

const TITLE: &str = concat!(
    " ▄████████    ▄█    █▄     ▄█   ▄█    █▄     ▄████████  ▄█          ▄████████ ▄██   ▄         ▄█   ▄█  \r\n",
    " ███    ███   ███    ███   ███  ███    ███   ███    ███ ███         ███    ███ ███   ██▄      ███  ███ \r\n",
    " ███    █▀    ███    ███   ███▌ ███    ███   ███    ███ ███         ███    ███ ███▄▄▄███      ███▌ ███▌\r\n",
    " ███         ▄███▄▄▄▄███▄▄ ███▌ ███    ███   ███    ███ ███        ▄███▄▄▄▄██▀ ▀▀▀▀▀▀███      ███▌ ███▌\r\n",
    " ███        ▀▀███▀▀▀▀███▀  ███▌ ███    ███ ▀███████████ ███       ▀▀███▀▀▀▀▀   ▄██   ███      ███▌ ███▌\r\n",
    " ███    █▄    ███    ███   ███  ███    ███   ███    ███ ███       ▀███████████ ███   ███      ███  ███ \r\n",
    " ███    ███   ███    ███   ███  ███    ███   ███    ███ ███▌    ▄   ███    ███ ███   ███      ███  ███ \r\n",
    " ████████▀    ███    █▀    █▀    ▀██████▀    ███    █▀  █████▄▄██   ███    ███  ▀█████▀       █▀   █▀  \r\n",
    "                                                        ▀           ███    ███                         \r\n",
);

const SUBTITLE: &str = concat!(
    " ███    █▄  ███▄▄▄▄    ▄████████    ▄█    █▄       ▄████████  ▄█  ███▄▄▄▄      ▄████████ ████████▄     \r\n",
    " ███    ███ ███▀▀▀██▄ ███    ███   ███    ███     ███    ███ ███  ███▀▀▀██▄   ███    ███ ███   ▀███    \r\n",
    " ███    ███ ███   ███ ███    █▀    ███    ███     ███    ███ ███▌ ███   ███   ███    █▀  ███    ███    \r\n",
    " ███    ███ ███   ███ ███         ▄███▄▄▄▄███▄▄   ███    ███ ███▌ ███   ███  ▄███▄▄▄     ███    ███    \r\n",
    " ███    ███ ███   ███ ███        ▀▀███▀▀▀▀███▀  ▀███████████ ███▌ ███   ███ ▀▀███▀▀▀     ███    ███    \r\n",
    " ███    ███ ███   ███ ███    █▄    ███    ███     ███    ███ ███  ███   ███   ███    █▄  ███    ███    \r\n",
    " ███    ███ ███   ███ ███    ███   ███    ███     ███    ███ ███  ███   ███   ███    ███ ███   ▄███    \r\n",
    " ████████▀   ▀█   █▀  ████████▀    ███    █▀      ███    █▀  █▀    ▀█   █▀    ██████████ ████████▀     ",
);

pub(crate) fn print_banner() {
    sinfo!(("\r\n"), palette("title"), "{PADDING}\r\n{TITLE}",
        palette("subtitle"), "{SUBTITLE}\r\n{PADDING}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_has_matching_padding_and_indented_artwork() {
        assert!(PADDING.chars().all(|ch| ch == ' '));
        let artwork: Vec<_> = TITLE.lines().chain(SUBTITLE.lines()).collect();
        assert_eq!(artwork.len(), 17);
        assert!(artwork.iter().all(|line| line.starts_with(' ')));
        assert_eq!(PADDING.len(), artwork.iter().map(|line| line.chars().count()).max().unwrap());
        assert!(!SUBTITLE.ends_with('\n'), "bottom padding supplies the newline after the artwork");
    }
}
