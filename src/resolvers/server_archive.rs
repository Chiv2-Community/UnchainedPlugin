use std::{collections::HashMap, ffi::c_void};

const CONFIGURE_PATTERN: &str = "48 89 5C 24 08 57 48 83 EC 20 65 48 8B 04 25 58 00 00 00 48 8B F9 8B 15 ?? ?? ?? ?? B9 78 00 00 00 4C 8B 04 D0 42 8B 04 01 39 05 ?? ?? ?? ?? 0F 8F BD 00 00 00";

define_pattern_resolver!(ConfigureServerArchive, [CONFIGURE_PATTERN]);

type ArchiveCallback = unsafe extern "C" fn(*mut c_void, *mut c_void);

// An unbound callback selects native unsigned reader and precacher paths.
CREATE_HOOK!(ConfigureServerArchive, INACTIVE, NONE, (),
(callback: Option<ArchiveCallback>), {});

// Cache upgrades run under the loader lock; the regular scanner uses Rayon.
pub fn resolve_missing(
    offsets: &mut HashMap<String, u64>,
) -> Result<(), Box<dyn std::error::Error>> {
    if offsets
        .get("ConfigureServerArchive")
        .is_some_and(|offset| *offset != 0)
    {
        return Ok(());
    }
    let image = patternsleuth::process::internal::read_image()?;
    let section = image
        .memory
        .sections()
        .iter()
        .find(|section| section.name() == ".text")
        .ok_or("No executable text section found")?;
    let pattern = patternsleuth::scanner::Pattern::new(CONFIGURE_PATTERN)?;
    let index = unique_match(&pattern, section.data())?;
    let offset = section.address() + index - image.base_address;
    offsets.insert("ConfigureServerArchive".to_string(), offset as u64);
    Ok(())
}

fn unique_match(
    pattern: &patternsleuth::scanner::Pattern,
    data: &[u8],
) -> Result<usize, Box<dyn std::error::Error>> {
    let mut matches = data
        .windows(pattern.simple.len())
        .enumerate()
        .filter_map(|(index, _)| pattern.is_match(data, 0, index).then_some(index));
    let index = matches.next().ok_or("Server archive pattern not found")?;
    if matches.next().is_some() {
        return Err("Server archive pattern is ambiguous".into());
    }
    Ok(index)
}

pub unsafe fn enable() -> Result<(), Box<dyn std::error::Error>> {
    o_ConfigureServerArchive.enable()?;
    crate::sinfo!(f; "ConfigureServerArchive enabled");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequential_match_requires_unique_bounded_match() {
        let pattern = patternsleuth::scanner::Pattern::new("12 ?? 34").unwrap();
        assert_eq!(unique_match(&pattern, &[0, 0x12, 0xff, 0x34]).unwrap(), 1);
        assert!(unique_match(&pattern, &[0x12, 0xff]).is_err());
        assert!(unique_match(&pattern, &[0x12, 0, 0x34, 0x12, 1, 0x34]).is_err());
    }

    #[test]
    fn registration_does_not_invoke_callback() {
        unsafe extern "C" fn callback(_: *mut c_void, _: *mut c_void) {
            panic!("Archive callback must remain unregistered");
        }
        let _: unsafe extern "C" fn(Option<ArchiveCallback>) = ConfigureServerArchive_detour_fkt;
        unsafe {
            ConfigureServerArchive_detour_fkt(Some(callback));
            ConfigureServerArchive_detour_fkt(None);
        }
    }

    #[test]
    fn cached_offset_skips_scan() {
        let mut offsets = HashMap::from([("ConfigureServerArchive".to_string(), 1)]);
        resolve_missing(&mut offsets).unwrap();
        assert_eq!(offsets["ConfigureServerArchive"], 1);
    }
}
