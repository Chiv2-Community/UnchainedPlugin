mod provenance {
    include!(concat!(env!("OUT_DIR"), "/provenance.rs"));
}

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

const DIRTY_BUILD: &str = concat!(
    "                                                                                                        \r\n",
    " ·▄▄▄▄   ■   ▄▄▄   ▄▄▄▄▄  ▄· ▄▌      ▄▄▄▄·  ▄∙ ▄▌ ■   ▄▄▌   ·▄▄▄▄                                       \r\n",
    " ██■ ██  ██  ▀▄ █· ∙██   ▐█■██▌      ▐█ ▀█■ █■██▌ ██  ██∙   ██■ ██                                      \r\n",
    " ▐█· ▐█▌ ▐█· ▐▀▀▄   ▐█.■ ▐█▌▐█■      ▐█▀▀█▄ █▌▐█▌ ▐█· ██■   ▐█· ▐█▌                                     \r\n",
    " ██. ██  ▐█▌ ▐█∙█▌  ▐█▌·  ▐█▀·.      ██▄■▐█ ▐█▄█▌ ▐█▌ ▐█▌▐▌ ██. ██                                      \r\n",
    " ▀▀▀▀▀∙  ▀▀▀ .▀  ▀  ▀▀▀    ▀ ∙       ·▀▀▀▀   ▀▀▀  ▀▀▀ .▀▀▀  ▀▀▀▀▀∙                                      \r\n",
    "                                                                                                        ",
);

pub(crate) fn print_banner() {
    sinfo!(("\r\n"), palette("title"), "{TITLE}", palette("subtitle"), "{SUBTITLE}");
    if provenance::DIRTY {
        swarn!(("\r\n"), palette("dirty_build"), "{DIRTY_BUILD}");
        swarn!("Revision: {}", provenance::COMMIT);
        print_modifications();
    } else {
        sinfo!("{}", provenance::REVISION);
    }
}

fn print_modifications() {
    if provenance::UNCOMMITTED_CHANGES.is_empty() {
        swarn!("Uncommitted change details unavailable.");
    } else {
        swarn!("Uncommitted changes:");
        for change in provenance::UNCOMMITTED_CHANGES {
            swarn!("  {change}");
        }
    }
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

    #[test]
    fn dirty_banner_matches_title_width_with_padding_on_all_sides() {
        let lines: Vec<_> = DIRTY_BUILD.lines().collect();
        assert_eq!(lines.len(), 7);
        assert_eq!(lines[0], lines[6]);
        assert_eq!(lines[0], TITLE.lines().next().unwrap());
        for line in &lines {
            assert_eq!(line.chars().count(), 104);
            assert!(line.starts_with(' ') && line.ends_with(' '));
        }
        assert!(lines[0].chars().all(|ch| ch == ' '));
    }
}
