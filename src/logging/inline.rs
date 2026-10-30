use std::fmt;

#[doc(hidden)]
pub enum Segment<'a> {
    Text(fmt::Arguments<'a>),
    Palette(&'a str),
}

#[doc(hidden)]
pub struct Message<'a>(pub &'a [Segment<'a>], pub u64);

// A per-thread token prevents reentrant formatting from switching another record's writer.
#[doc(hidden)]
pub fn next_id() -> u64 {
    thread_local! { static NEXT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) }; }
    NEXT.with(|next| {
        let id = next.get().wrapping_add(1);
        next.set(id);
        id
    })
}

impl fmt::Display for Message<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for segment in self.0 {
            match segment {
                Segment::Text(args) => write!(f, "{args}")?,
                Segment::Palette(name) => super::log4rs::palette::inline_color(
                    self.1, name),
            }
        }
        Ok(())
    }
}
