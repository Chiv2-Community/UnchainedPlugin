//! Discord embed-size accounting, independent of transport and queueing.
use serenity::all::CreateEmbed;

const MAX_EMBEDS: usize = 10;
const MAX_CHARACTERS: usize = 6000;
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
        self.embeds.len() == MAX_EMBEDS
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

    #[test]
    fn truncation_preserves_exact_limits_and_reports_utf16_length() {
        for input in [
            String::new(),
            "short 🩸 text".into(),
            "界".repeat(MAX_DESCRIPTION),
            format!("{}🩸", "a".repeat(MAX_DESCRIPTION - 2)),
            format!("{}🩸", "a".repeat(MAX_DESCRIPTION - 1)),
            "a".repeat(MAX_DESCRIPTION + 1),
            "🩸".repeat(MAX_DESCRIPTION),
        ] {
            let original_units = input.encode_utf16().count();
            let (output, units) = truncate_description(input.clone());
            assert_eq!(units, output.encode_utf16().count());
            assert!(units <= MAX_DESCRIPTION);
            if original_units <= MAX_DESCRIPTION {
                assert_eq!(output, input);
            } else {
                assert!(output.ends_with('…'));
                assert!(input.starts_with(output.trim_end_matches('…')));
            }
        }
    }

    #[test]
    fn batch_enforces_embed_and_character_limits_and_resets() {
        let mut batch = Batch::default();
        for _ in 0..MAX_EMBEDS {
            let entry = LogEmbed {
                embed: CreateEmbed::new(),
                characters: 600,
            };
            assert!(batch.fits(&entry));
            batch.push(entry);
        }
        assert!(batch.is_full());
        assert!(!batch.fits(&LogEmbed {
            embed: CreateEmbed::new(),
            characters: 1
        }));
        assert_eq!(batch.take().len(), MAX_EMBEDS);
        assert!(batch.is_empty());
        batch.push(LogEmbed {
            embed: CreateEmbed::new(),
            characters: 3500,
        });
        assert!(!batch.fits(&LogEmbed {
            embed: CreateEmbed::new(),
            characters: 3500
        }));
    }
}
