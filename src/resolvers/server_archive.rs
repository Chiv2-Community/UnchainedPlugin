use std::{collections::HashMap, ffi::c_void, sync::OnceLock};

const PREPARE_PATTERN: &str = "48 89 5C 24 08 48 89 6C 24 10 48 89 74 24 18 57 48 83 EC 20 80 B9 48 01 00 00 00 49 8B F0 48 8B FA 48 8B D9 75 34 E8 ?? ?? ?? ?? 48 8B C8";
const DISPATCH_PATTERN: &str = "4C 8B DC 53 55 56 57 41 54 41 55 41 57 48 81 EC F0 00 00 00 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 84 24 E0 00 00 00 0F 57 C0 49 63 E9";
const COMPLETE_PATTERN: &str = "48 89 5C 24 08 48 89 6C 24 10 48 89 74 24 18 57 41 56 41 57 48 83 EC 20 48 8D 59 08 49 63 F9 48 8B F1 49 8B E8 48 8B CB 44 0F B6 F2 FF 15 ?? ?? ?? ??";

define_pattern_resolver!(PrepareServerArchive, {
    STEAM: [PREPARE_PATTERN]
});
define_pattern_resolver!(DispatchServerRead, {
    STEAM: [DISPATCH_PATTERN]
});
define_pattern_resolver!(CompleteServerRead, {
    STEAM: [COMPLETE_PATTERN]
});

type ReadCompletion = unsafe extern "C" fn(*mut c_void, bool, *mut c_void, i32);
static READ_COMPLETION: OnceLock<ReadCompletion> = OnceLock::new();

// Cache upgrades must not wait for the background scan while the game resumes.
// Use only the sequential matcher here: the regular scanner starts Rayon workers.
pub fn resolve_missing(
    offsets: &mut HashMap<String, u64>,
    platform: super::PlatformType,
) -> Result<(), Box<dyn std::error::Error>> {
    if completion_offset(offsets).is_ok() {
        return Ok(());
    }
    if platform != super::PlatformType::STEAM {
        return Err("Server archive patterns are only verified for Steam".into());
    }
    let image = patternsleuth::process::internal::read_image()?;
    let section = image
        .memory
        .sections()
        .iter()
        .find(|section| section.name() == ".text")
        .ok_or("No executable text section found")?;
    let mut resolved = Vec::new();
    for (name, signature) in [
        ("PrepareServerArchive", PREPARE_PATTERN),
        ("DispatchServerRead", DISPATCH_PATTERN),
        ("CompleteServerRead", COMPLETE_PATTERN),
    ] {
        if offsets.get(name).is_some_and(|offset| *offset != 0) {
            continue;
        }
        let pattern = patternsleuth::scanner::Pattern::new(signature)?;
        let index = unique_match(&pattern, section.data())?;
        // Section addresses use the PE image base, just as the regular scanner does.
        let offset = section.address() + index - image.base_address;
        resolved.push((name.to_string(), offset as u64));
    }
    offsets.extend(resolved);
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

CREATE_HOOK!(PrepareServerArchive, INACTIVE, NONE, *mut c_void,
(this: *mut c_void, reader: *mut c_void, filename: *const u16), {
    reader
});

CREATE_HOOK!(DispatchServerRead, INACTIVE, NONE, (),
(this: *mut c_void, cancelled: bool, request: *mut c_void, index: i32), {
    // Use normal completion to retain callback and buffer ownership semantics.
    READ_COMPLETION.get().expect("Server read completion not initialized")(
        this, cancelled, request, index,
    );
});

fn completion_offset(offsets: &HashMap<String, u64>) -> Result<u64, Box<dyn std::error::Error>> {
    for name in [
        "PrepareServerArchive",
        "DispatchServerRead",
        "CompleteServerRead",
    ] {
        if !offsets.get(name).is_some_and(|offset| *offset != 0) {
            return Err(format!("No address found for {name}").into());
        }
    }
    Ok(offsets["CompleteServerRead"])
}

/// Initialize the dependency before inventory attachment; enable both hooks afterward.
pub unsafe fn initialize(offsets: &HashMap<String, u64>) -> Result<(), Box<dyn std::error::Error>> {
    let offset = completion_offset(offsets)?;
    let completion: ReadCompletion = std::mem::transmute(*super::BASE_ADDR + offset as usize);
    READ_COMPLETION
        .set(completion)
        .map_err(|_| "Server read completion already initialized")?;
    Ok(())
}

pub unsafe fn enable() -> Result<(), Box<dyn std::error::Error>> {
    // Dispatch must be ready before archives can be prepared without metadata.
    o_DispatchServerRead.enable()?;
    if let Err(error) = o_PrepareServerArchive.enable() {
        o_DispatchServerRead.disable()?;
        return Err(error.into());
    }
    crate::sinfo!(f; "PrepareServerArchive and DispatchServerRead enabled");
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
    fn reader_identity_including_null() {
        let mut byte = 0u8;
        let pointer = (&mut byte as *mut u8).cast();
        for reader in [std::ptr::null_mut(), pointer] {
            assert_eq!(
                unsafe {
                    PrepareServerArchive_detour_fkt(std::ptr::null_mut(), reader, std::ptr::null())
                },
                reader
            );
        }
    }

    #[test]
    fn completion_requires_every_offset() {
        let all: HashMap<_, _> = [
            ("PrepareServerArchive".to_string(), 1),
            ("DispatchServerRead".to_string(), 2),
            ("CompleteServerRead".to_string(), 3),
        ]
        .into();
        assert_eq!(completion_offset(&all).unwrap(), 3);
        for name in all.keys() {
            let mut missing = all.clone();
            missing.remove(name);
            assert!(completion_offset(&missing).is_err());
            missing.insert(name.clone(), 0);
            assert!(completion_offset(&missing).is_err());
        }
    }

    #[test]
    fn dispatch_preserves_arguments_and_runs_completion() {
        struct Request {
            owner: *mut c_void,
            index: i32,
            cancelled: bool,
            calls: usize,
        }
        unsafe extern "C" fn complete(
            this: *mut c_void,
            cancelled: bool,
            request: *mut c_void,
            index: i32,
        ) {
            let request = &mut *request.cast::<Request>();
            request.owner = this;
            request.index = index;
            request.cancelled = cancelled;
            request.calls += 1;
        }
        let completion: unsafe extern "C" fn(*mut c_void, bool, *mut c_void, i32) = complete;
        let _: unsafe extern "C" fn(*mut c_void, bool, *mut c_void, i32) =
            DispatchServerRead_detour_fkt;
        READ_COMPLETION.set(completion).unwrap();
        let mut owner = 0u8;
        let owner = (&mut owner as *mut u8).cast();
        let mut request = Request {
            owner: std::ptr::null_mut(),
            index: 0,
            cancelled: false,
            calls: 0,
        };
        for (calls, (cancelled, index)) in
            [(false, i32::MIN), (true, -1), (false, 0), (true, i32::MAX)]
                .into_iter()
                .enumerate()
        {
            unsafe {
                DispatchServerRead_detour_fkt(
                    owner,
                    cancelled,
                    (&mut request as *mut Request).cast(),
                    index,
                )
            };
            assert_eq!(request.owner, owner);
            assert_eq!(request.index, index);
            assert_eq!(request.cancelled, cancelled);
            assert_eq!(request.calls, calls + 1);
        }
    }
}
