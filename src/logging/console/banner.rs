const TITLE: &str = concat!(
    "                                                                                                        \r\n",
    " ▄████████    ▄█    █▄     ▄█   ▄█    █▄     ▄████████  ▄█          ▄████████ ▄██   ▄         ▄█   ▄█   \r\n",
    " ███    ███   ███    ███   ███  ███    ███   ███    ███ ███         ███    ███ ███   ██▄      ███  ███  \r\n",
    " ███    █▀    ███    ███   ███▌ ███    ███   ███    ███ ███         ███    ███ ███▄▄▄███      ███▌ ███▌ \r\n",
    " ███         ▄███▄▄▄▄███▄▄ ███▌ ███    ███   ███    ███ ███        ▄███▄▄▄▄██▀ ▀▀▀▀▀▀███      ███▌ ███▌ \r\n",
    " ███        ▀▀███▀▀▀▀███▀  ███▌ ███    ███ ▀███████████ ███       ▀▀███▀▀▀▀▀   ▄██   ███      ███▌ ███▌ \r\n",
    " ███    █▄    ███    ███   ███  ███    ███   ███    ███ ███       ▀███████████ ███   ███      ███  ███  \r\n",
    " ███    ███   ███    ███   ███  ███    ███   ███    ███ ███▌    ▄   ███    ███ ███   ███      ███  ███  \r\n",
    " ████████▀    ███    █▀    █▀    ▀██████▀    ███    █▀  █████▄▄██   ███    ███  ▀█████▀       █▀   █▀   \r\n",
    "                                                        ▀           ███    ███                          \r\n",
);

const SUBTITLE: &str = concat!(
    " ███    █▄  ███▄▄▄▄    ▄████████    ▄█    █▄       ▄████████  ▄█  ███▄▄▄▄      ▄████████ ████████▄      \r\n",
    " ███    ███ ███▀▀▀██▄ ███    ███   ███    ███     ███    ███ ███  ███▀▀▀██▄   ███    ███ ███   ▀███     \r\n",
    " ███    ███ ███   ███ ███    █▀    ███    ███     ███    ███ ███▌ ███   ███   ███    █▀  ███    ███     \r\n",
    " ███    ███ ███   ███ ███         ▄███▄▄▄▄███▄▄   ███    ███ ███▌ ███   ███  ▄███▄▄▄     ███    ███     \r\n",
    " ███    ███ ███   ███ ███        ▀▀███▀▀▀▀███▀  ▀███████████ ███▌ ███   ███ ▀▀███▀▀▀     ███    ███     \r\n",
    " ███    ███ ███   ███ ███    █▄    ███    ███     ███    ███ ███  ███   ███   ███    █▄  ███    ███     \r\n",
    " ███    ███ ███   ███ ███    ███   ███    ███     ███    ███ ███  ███   ███   ███    ███ ███   ▄███     \r\n",
    " ████████▀   ▀█   █▀  ████████▀    ███    █▀      ███    █▀  █▀    ▀█   █▀    ██████████ ████████▀      \r\n",
    "                                                                                                        ",
);

pub(crate) fn print_banner() {
    sinfo!(("\r\n"), palette("title"), "{TITLE}", palette("subtitle"), "{SUBTITLE}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_has_uniform_width_matching_padding_and_indented_artwork() {
        let mut title = TITLE.lines();
        let mut subtitle = SUBTITLE.lines();
        let top = title.next().unwrap();
        let bottom = subtitle.next_back().unwrap();
        assert_eq!(top, bottom);
        assert!(top.chars().all(|ch| ch == ' '));
        let artwork: Vec<_> = title.chain(subtitle).collect();
        assert_eq!(artwork.len(), 17);
        assert!(artwork.iter().all(|line| line.starts_with(' ')));
        let width = top.chars().count();
        assert_eq!(width, 104);
        for (index, line) in TITLE.lines().chain(SUBTITLE.lines()).enumerate() {
            assert!(line.ends_with(' '), "banner row {} lacks right padding", index + 1);
            assert_eq!(line.chars().count(), width, "banner row {} has a different width", index + 1);
        }
    }
}
