use std::{borrow::Cow, io::IsTerminal};
use rand::{rngs::StdRng, seq::SliceRandom, SeedableRng};

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
        let emoji_supported = std::io::stdout().is_terminal()
            && !super::should_strip_console_emojis(log4rs::append::console::Target::Stdout);
        let dirty_build = dirty_banner(provenance::BANNER_SEED, emoji_supported);
        swarn!(("\r\n"), palette("dirty_build"), "{dirty_build}");
        swarn!("Revision: {}", provenance::COMMIT);
        print_modifications();
    } else {
        sinfo!("{}", provenance::REVISION);
    }
}

fn dirty_banner(seed: u64, emoji_supported: bool) -> Cow<'static, str> {
    if !emoji_supported {
        return Cow::Borrowed(DIRTY_BUILD);
    }
    // Replace two terminal cells per fly, keeping space around the artwork and edges.
    let bytes = DIRTY_BUILD.as_bytes();
    let mut gaps: Vec<_> = DIRTY_BUILD.match_indices("  ")
        .map(|(offset, _)| offset)
        .filter(|&offset| offset > 0 && bytes[offset - 1] == b' '
            && bytes.get(offset + 2) == Some(&b' '))
        .collect();
    gaps.shuffle(&mut StdRng::seed_from_u64(seed));
    gaps.truncate(10);
    gaps.sort_unstable();
    let mut decorated = String::with_capacity(DIRTY_BUILD.len() + gaps.len() * 2);
    let mut start = 0;
    for offset in gaps {
        decorated.push_str(&DIRTY_BUILD[start..offset]);
        decorated.push('🪰');
        start = offset + 2;
    }
    decorated.push_str(&DIRTY_BUILD[start..]);
    Cow::Owned(decorated)
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
    fn flies_are_seeded_and_replace_only_whitespace() {
        let banner = dirty_banner(1, true);
        assert_eq!(banner, dirty_banner(1, true));
        assert_ne!(banner, dirty_banner(2, true));
        assert_eq!(banner.matches('🪰').count(), 10);
        assert_eq!(banner.replace('🪰', "  "), DIRTY_BUILD);
        assert!(banner.lines().all(|line| line.starts_with(' ') && line.ends_with(' ')));
    }

    #[test]
    fn unsupported_terminals_keep_the_plain_banner() {
        assert_eq!(dirty_banner(1, false), DIRTY_BUILD);
    }

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
