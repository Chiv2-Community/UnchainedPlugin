use log4rs::append::console::Target;
use log4rs::encode::writer::console::ConsoleWriter;

mod banner;
pub(crate) use banner::print_banner;

static NO_SPAWN_CONSOLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static ATTACH_STDIO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static NO_STD_OUTPUT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[cfg(windows)]
static OWN_CONSOLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn attach_stdio() {
    ATTACH_STDIO.store(true, std::sync::atomic::Ordering::Relaxed);
    #[cfg(windows)]
    NO_STD_OUTPUT.store(!windows_backend::attach_parent_output(), std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn stdio_attached() -> bool {
    ATTACH_STDIO.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(windows)]
pub(crate) fn spawn_console() {
    windows_backend::spawn_console();
}

pub(crate) fn disable_console_spawning() {
    NO_SPAWN_CONSOLE.store(true, std::sync::atomic::Ordering::Relaxed);
}

pub(crate) fn console_spawning_disabled() -> bool {
    NO_SPAWN_CONSOLE.load(std::sync::atomic::Ordering::Relaxed)
}

pub(crate) fn report_attachment_failure() {
    if NO_STD_OUTPUT.load(std::sync::atomic::Ordering::Relaxed) {
        swarn!(fn; "Could not attach inherited output or the parent console.");
    }
}

#[cfg(windows)]
#[path = "windows.rs"]
mod windows_backend;

const PALETTE_SLOTS: [u8; 15] = [12, 14, 9, 7, 8, 1, 2, 3, 4, 5, 6, 10, 11, 13, 0];
const BASIC_COLORS: [u32; 16] = [
    0x000000, 0x800000, 0x008000, 0x808000, 0x000080, 0x800080, 0x008080, 0xc0c0c0,
    0x808080, 0xff0000, 0x00ff00, 0xffff00, 0x0000ff, 0xff00ff, 0x00ffff, 0xffffff,
];

fn nearest_slots(colors: [u32; 16], palette: &[[u8; 3]]) -> Vec<u8> {
    palette.iter().map(|rgb| {
        colors.iter().enumerate().min_by_key(|(_, color)| {
            rgb.iter().enumerate().map(|(i, channel)| {
                let difference = i32::from(*channel) - ((**color >> (i * 8)) & 255) as i32;
                difference * difference
            }).sum::<i32>()
        }).unwrap().0 as u8
    }).collect()
}

#[cfg(any(windows, test))]
fn fallback_palette(mut colors: [u32; 16], attributes: u16, palette: &[[u8; 3]]) -> ([u32; 16], u16) {
    let background = ((attributes >> 4) & 15) as u8;
    let normal = if background == 15 { 0 } else { 15 };
    colors[normal as usize] = colors[(attributes & 15) as usize];
    let slots = PALETTE_SLOTS.into_iter().filter(|slot| *slot != background && *slot != normal);
    for (rgb, slot) in palette.iter().zip(slots) {
        let [r, g, b] = *rgb;
        colors[usize::from(slot)] = u32::from(r) | (u32::from(g) << 8) | (u32::from(b) << 16);
    }
    (colors, (attributes & !0x0f) | u16::from(normal))
}

#[cfg(any(windows, test))]
fn use_truecolor(vt_enabled: bool, wine: bool, windows_terminal: bool) -> bool {
    vt_enabled && (!wine || windows_terminal)
}

pub(crate) fn configure_palette(target: Target, colors: &[[u8; 3]]) -> Option<(bool, Vec<u8>)> {
    let writer = match target {
        Target::Stdout => ConsoleWriter::stdout(),
        Target::Stderr => ConsoleWriter::stderr(),
    };
    writer?;
    if colors.is_empty() { return Some((false, Vec::new())); }
    #[cfg(windows)]
    { Some(windows_backend::configure(target, colors)) }
    #[cfg(not(windows))]
    { Some((true, nearest_slots(BASIC_COLORS, colors))) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_requires_vt_and_avoids_wine_graphical_console() {
        assert!(use_truecolor(true, false, false));
        assert!(use_truecolor(true, true, true));
        assert!(!use_truecolor(true, true, false));
        assert!(!use_truecolor(false, false, true));
    }

    #[test]
    fn legacy_palette_is_bounded_and_preserves_normal_text_and_background() {
        let original = [0x101010; 16];
        let palette: Vec<_> = (0..40).map(|i| [i * 5, i * 3, i * 2]).collect();
        let (colors, attributes) = fallback_palette(original, 0x47, &palette);
        assert_eq!(attributes, 0x4f);
        assert_eq!(colors[15], original[7]);
        let slots: Vec<_> = PALETTE_SLOTS.into_iter().filter(|slot| *slot != 4).collect();
        assert_eq!(nearest_slots(colors, &palette[..14]), slots);
        assert_eq!(colors[4], original[4]);
        assert_eq!(nearest_slots(colors, &palette).len(), 40);
        assert_eq!(fallback_palette(colors, attributes, &palette), (colors, attributes));
        for background in 0..16 {
            let original = std::array::from_fn(|i| i as u32 * 0x111111);
            let attributes = background << 4 | 7;
            let (colors, attributes) = fallback_palette(original, attributes, &palette);
            assert_eq!(colors[background as usize], original[background as usize]);
            assert_eq!(colors[(attributes & 15) as usize], original[7]);
        }
    }
}
