use std::path::Path;
use std::sync::OnceLock;

use crate::tools::hook_globals::cli_args;
use crate::ue::{FString, TArray};

struct AdditionalPakDirectory {
    path: String,
}

static ADDITIONAL_PAK_DIRECTORY: OnceLock<AdditionalPakDirectory> = OnceLock::new();

fn normalize_directory(path: &Path) -> std::io::Result<String> {
    if !path.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "pak directory does not exist or is not a directory",
        ));
    }
    let absolute = std::path::absolute(path)?;
    let mut path = absolute.to_str().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "pak directory is not Unicode")
    })?.replace('\\', "/");
    if !path.ends_with('/') {
        path.push('/');
    }
    Ok(path)
}

pub(crate) fn initialize() {
    let Some(path) = &cli_args().additional_pak_dir else { return };
    let path = match normalize_directory(Path::new(path)) {
        Ok(path) => path,
        Err(error) => {
            crate::serror!("Cannot load additional pak directory {path:?}: {error}");
            return;
        }
    };
    if !crate::ue::gmalloc_resolved() {
        crate::serror!("Cannot load additional pak directory: GMalloc was not resolved");
        return;
    }
    let _ = ADDITIONAL_PAK_DIRECTORY.set(AdditionalPakDirectory { path });
}

// FPakPlatformFile::GetPakFolders(const TCHAR* CmdLine, TArray<FString>& OutPakFolders).
// Verified against Steam: appends the project, engine and saved Paks directories.
define_pattern_resolver!(GetPakFolders, [
    "48 89 5C 24 08 48 89 74 24 10 48 89 7C 24 18 4C 89 74 24 20 55 48 8B EC 48 83 EC 40 48 8D 4D F0 48 8B DA"
]);

CREATE_HOOK!(GetPakFolders, { || ADDITIONAL_PAK_DIRECTORY.get().is_some() }, NONE, (),
    (cmd_line: *const u16, folders: *mut TArray<FString>), {
        CALL_ORIGINAL!(GetPakFolders(cmd_line, folders));
        let Some(config) = ADDITIONAL_PAK_DIRECTORY.get() else { return };
        if folders.is_null() {
            return;
        }
        let Some(allocator) = crate::ue::try_gmalloc() else {
            crate::serror!("Cannot load additional pak directory: GMalloc is not initialized");
            return;
        };
        let folders = unsafe { &mut *folders };
        if folders.as_slice().iter().any(|folder| folder.to_string() == config.path) {
            return;
        }
        let path: Vec<u16> = config.path.encode_utf16().chain(std::iter::once(0)).collect();
        // Both the FString and outer array must remain owned by Unreal, which
        // frees them after discovery. Never give the engine a Rust allocation.
        unsafe {
            let path = FString::from_slice_in(&path, allocator);
            folders.push_in(path, allocator);
        }
        crate::sinfo!("Added pak search directory: {}", config.path);
    }
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_is_absolute_with_trailing_slash() {
        let path = normalize_directory(Path::new(".")).unwrap();
        assert!(Path::new(&path).is_absolute());
        assert!(path.ends_with('/'));
        assert!(!path.contains('\\'));
    }

    #[test]
    fn missing_directory_is_rejected() {
        assert!(normalize_directory(Path::new("Cargo.toml/missing")).is_err());
    }

    #[test]
    fn file_is_rejected() {
        assert!(normalize_directory(Path::new("Cargo.toml")).is_err());
    }
}
