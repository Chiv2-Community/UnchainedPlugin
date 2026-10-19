use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::tools::hook_globals::cli_args;
use crate::ue::{FString, TArray};

// FPakPlatformFile::GetPakFolders(const TCHAR* CmdLine, TArray<FString>& OutPakFolders).
// Verified against Steam: appends the project, engine and saved Paks directories.
define_pattern_resolver!(GetPakFolders, [
    "48 89 5C 24 08 48 89 74 24 10 48 89 7C 24 18 4C 89 74 24 20 55 48 8B EC 48 83 EC 40 48 8D 4D F0 48 8B DA"
]);

CREATE_HOOK!(GetPakFolders, { || !directories().is_empty() }, NONE, (),
    (cmd_line: *const u16, folders: *mut TArray<FString>), {
        CALL_ORIGINAL!(GetPakFolders(cmd_line, folders));
        let paths = directories();
        if folders.is_null() {
            return;
        }
        if crate::ue::try_gmalloc().is_none() {
            crate::serror!("Cannot load additional pak directory: GMalloc is not initialized");
            return;
        }
        let folders = unsafe { &mut *folders };
        let mut seen = Vec::new();
        for folder in folders.as_slice() {
            insert_directory(&mut seen, Path::new(&folder.to_string()));
        }
        for path in paths {
            if !insert_directory(&mut seen, Path::new(path)) {
                continue;
            }
            // Both buffers transfer to Unreal, which frees them after discovery.
            folders.push(FString::from(path.as_str()));
            crate::sinfo!("Added pak search directory: {}", path);
        }
    }
);

static ADDITIONAL_PAK_DIRECTORIES: OnceLock<Vec<String>> = OnceLock::new();

fn normalize_directory(path: &Path) -> std::io::Result<String> {
    if !path.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "pak directory does not exist or is not a directory",
        ));
    }
    let absolute = std::path::absolute(path)?;
    let mut path = absolute
        .to_str()
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "pak directory is not Unicode",
            )
        })?
        .replace('\\', "/");
    if !path.ends_with('/') {
        path.push('/');
    }
    Ok(path)
}

fn normalize_directories(paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .filter_map(|path| match normalize_directory(Path::new(path)) {
            Ok(path) => Some(path),
            Err(error) => {
                crate::serror!("Cannot load additional pak directory {path:?}: {error}");
                None
            }
        })
        .collect()
}

fn directories() -> &'static [String] {
    ADDITIONAL_PAK_DIRECTORIES
        .get_or_init(|| normalize_directories(&cli_args().additional_pak_dirs))
}

fn same_directory_path(left: &Path, right: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::Globalization::{CompareStringOrdinal, CSTR_EQUAL};

        let left: Vec<u16> = left.as_os_str().encode_wide().collect();
        let right: Vec<u16> = right.as_os_str().encode_wide().collect();
        unsafe { CompareStringOrdinal(&left, &right, true) == CSTR_EQUAL }
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

fn insert_directory(seen: &mut Vec<PathBuf>, path: &Path) -> bool {
    // Compare resolved paths without changing the spelling passed to Unreal.
    let Ok(path) = std::fs::canonicalize(path).or_else(|_| std::path::absolute(path)) else {
        return true;
    };
    if seen
        .iter()
        .any(|existing| same_directory_path(existing, &path))
    {
        false
    } else {
        seen.push(path);
        true
    }
}

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

    #[test]
    fn multiple_directories_keep_order_and_skip_invalid_paths() {
        let paths = [".", "Cargo.toml", ".."].map(str::to_string);
        assert_eq!(
            normalize_directories(&paths),
            [
                normalize_directory(Path::new(".")).unwrap(),
                normalize_directory(Path::new("..")).unwrap(),
            ]
        );
    }

    #[test]
    fn native_relative_directory_matches_configured_absolute_directory() {
        let mut seen = Vec::new();
        assert!(insert_directory(&mut seen, Path::new(".")));
        let configured = normalize_directory(Path::new(".")).unwrap();
        assert!(!insert_directory(&mut seen, Path::new(&configured)));
        assert!(!insert_directory(&mut seen, Path::new("./")));
        assert_eq!(seen.len(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn windows_directory_case_and_separators_do_not_create_duplicates() {
        let mut seen = Vec::new();
        let path = normalize_directory(Path::new(".")).unwrap();
        assert!(insert_directory(&mut seen, Path::new(&path)));
        let alternate = path.to_ascii_uppercase().replace('/', "\\");
        assert!(!insert_directory(&mut seen, Path::new(&alternate)));
        assert_eq!(seen.len(), 1);
    }

    #[cfg(windows)]
    #[test]
    fn windows_path_comparison_uses_ordinal_unicode_case_rules() {
        assert!(same_directory_path(
            Path::new(r"C:\Mods\Ä"),
            Path::new(r"c:\mods\ä")
        ));
        assert!(!same_directory_path(
            Path::new(r"C:\Mods\Ä"),
            Path::new(r"C:\Mods\A")
        ));
    }

    #[test]
    fn distinct_directories_remain_in_input_order() {
        let mut seen = Vec::new();
        let paths = [".", "./", "..", "../.", "."];
        let added: Vec<_> = paths
            .into_iter()
            .filter(|path| insert_directory(&mut seen, Path::new(path)))
            .collect();
        assert_eq!(added, [".", ".."]);
    }

    #[test]
    fn unresolvable_native_directory_does_not_block_valid_directories() {
        let mut seen = Vec::new();
        assert!(insert_directory(&mut seen, Path::new("Cargo.toml/missing")));
        assert!(!insert_directory(
            &mut seen,
            Path::new("Cargo.toml/missing")
        ));
        assert!(insert_directory(&mut seen, Path::new(".")));
        assert!(!insert_directory(&mut seen, Path::new("./")));
    }
}
