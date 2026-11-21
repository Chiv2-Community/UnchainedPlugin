use object::SectionKind;
use patternsleuth::{
    Memory, NamedMemorySection,
    image::{Image, ImageType, pe::PEImage},
};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, unchained_macros::SelectionKey)]
pub enum Flavor { First, Second, Shared }

static FLAVOR: OnceLock<Flavor> = OnceLock::new();

pub fn current_flavor() -> Flavor {
    *FLAVOR.get().expect("fixture selection not initialized")
}

pub const BASE: u64 = 0x1_4000_0000;
pub const TEXT: u64 = BASE + 0x1000;

// Each exact child test gets its own immutable selection.
pub fn initialize_selection(flavor: Flavor) -> bool {
    if std::env::var_os("UNCHAINED_SELECTION_FIXTURE").is_some() {
        FLAVOR.set(flavor).unwrap();
        return true;
    }
    let thread = std::thread::current();
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", thread.name().unwrap(), "--nocapture"])
        .env("UNCHAINED_SELECTION_FIXTURE", "1")
        .status().unwrap();
    assert!(status.success());
    false
}

pub fn image(matches: &[usize]) -> Image<'static> {
    let mut text = vec![0x90; 0x80];
    text[..2].copy_from_slice(&[0x11, 0x22]);
    for &offset in matches {
        text[offset..offset + 2].copy_from_slice(&[0xAA, 0xBB]);
    }
    let pdata: Vec<u8> = [0x1000u32, 0x1040, 0x3000, 0x1040, 0x1080, 0x3004]
        .into_iter().flat_map(u32::to_le_bytes).collect();
    Image::from_memory(
        BASE,
        Memory::from_sections(vec![
            NamedMemorySection::new(".text".into(), TEXT, SectionKind::Text, text),
            NamedMemorySection::new(".pdata".into(), BASE + 0x2000, SectionKind::ReadOnlyData, pdata),
            NamedMemorySection::new(".xdata".into(), BASE + 0x3000, SectionKind::ReadOnlyData, vec![1, 0, 0, 0, 1, 0, 0, 0]),
        ]),
        ImageType::PEImage(PEImage {
            exception_directory_range: BASE + 0x2000..BASE + 0x2018,
            exception_children_cache: Default::default(),
        }),
    )
}
