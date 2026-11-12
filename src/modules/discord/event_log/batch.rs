//! Discord embed-size accounting, independent of transport and queueing.
use serenity::all::CreateEmbed;

const MAX_EMBEDS: usize = 10;
pub(crate) const MAX_CHARACTERS: usize = 6000;
const MAX_DESCRIPTION: usize = 4096;

pub(crate) struct LogEmbed {
    pub embed: CreateEmbed,
    pub characters: usize,
}

/// Return the description and its UTF-16 length in one bounded scan.
pub(crate) fn truncate_description(mut text: String) -> (String, usize) {
    let mut units = 0;
    let mut ellipsis_boundary = (0, 0);
    for (index, ch) in text.char_indices() {
        units += ch.len_utf16();
        if units < MAX_DESCRIPTION {
            ellipsis_boundary = (index + ch.len_utf8(), units);
        }
        if units > MAX_DESCRIPTION {
            let (end, kept_units) = ellipsis_boundary;
            text.truncate(end);
            text.push('…');
            return (text, kept_units + 1);
        }
    }
    (text, units)
}

#[derive(Default)]
pub(crate) struct Batch {
    embeds: Vec<CreateEmbed>,
    characters: usize,
}

impl Batch {
    pub fn len(&self) -> usize {
        self.embeds.len()
    }
    pub fn characters(&self) -> usize {
        self.characters
    }
    pub fn is_empty(&self) -> bool {
        self.embeds.is_empty()
    }
    pub fn is_full(&self) -> bool {
        self.embeds.len() >= MAX_EMBEDS || self.characters >= MAX_CHARACTERS
    }
    pub fn fits(&self, entry: &LogEmbed) -> bool {
        !self.is_full() && self.characters + entry.characters <= MAX_CHARACTERS
    }
    pub fn push(&mut self, entry: LogEmbed) {
        self.characters += entry.characters;
        self.embeds.push(entry.embed);
    }
    pub fn take(&mut self) -> Vec<CreateEmbed> {
        self.characters = 0;
        std::mem::replace(&mut self.embeds, Vec::with_capacity(MAX_EMBEDS))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case::test_case(String::new(); "empty")]
    #[test_case::test_case("short 🩸 text".into(); "short_unicode")]
    #[test_case::test_case("界".repeat(MAX_DESCRIPTION); "bmp_at_limit")]
    #[test_case::test_case(format!("{}🩸", "a".repeat(MAX_DESCRIPTION - 2)); "surrogate_pair_at_limit")]
    fn descriptions_within_limit_remain_unchanged(input: String) {
        let units = input.encode_utf16().count();
        assert_eq!(truncate_description(input.clone()), (input, units));
    }

    #[test_case::test_case(format!("{}🩸", "a".repeat(MAX_DESCRIPTION - 1)), format!("{}…", "a".repeat(MAX_DESCRIPTION - 1)); "pair_crosses_limit")]
    #[test_case::test_case("a".repeat(MAX_DESCRIPTION + 1), format!("{}…", "a".repeat(MAX_DESCRIPTION - 1)); "bmp_over_limit")]
    #[test_case::test_case("🩸".repeat(MAX_DESCRIPTION), format!("{}…", "🩸".repeat(MAX_DESCRIPTION / 2 - 1)); "pairs_over_limit")]
    fn oversized_descriptions_reserve_space_for_ellipsis(input: String, expected: String) {
        let units = expected.encode_utf16().count();
        assert_eq!(truncate_description(input), (expected, units));
    }

    #[test_case::test_case(MAX_EMBEDS, 1, 1, true; "embed_limit")]
    #[test_case::test_case(1, MAX_CHARACTERS, 1, true; "character_limit")]
    #[test_case::test_case(1, 3500, 3500, false; "combined_character_limit")]
    fn batch_rejects_entries_exceeding_limits(count: usize, characters: usize, incoming: usize, full: bool) {
        let mut batch = Batch::default();
        for _ in 0..count {
            let entry = LogEmbed { embed: CreateEmbed::new(), characters };
            assert!(batch.fits(&entry));
            batch.push(entry);
        }
        assert_eq!(batch.is_full(), full);
        assert!(!batch.fits(&LogEmbed { embed: CreateEmbed::new(), characters: incoming }));
    }

    #[test]
    fn taking_batch_resets_embed_and_character_counts() {
        let mut batch = Batch::default();
        batch.push(LogEmbed { embed: CreateEmbed::new(), characters: MAX_CHARACTERS });
        assert_eq!(batch.take().len(), 1);
        assert!(batch.is_empty());
        assert_eq!(batch.characters(), 0);
    }
}
