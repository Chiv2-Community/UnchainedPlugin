use windows::core::{s, w, HRESULT, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, GetHandleInformation, ERROR_FILE_NOT_FOUND, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0};
use windows::Win32::System::Console::*;
use windows::Win32::System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Registry::*;
use windows::Win32::System::Threading::{CreateMutexW, GetStartupInfoW, ReleaseMutex, WaitForSingleObject, STARTUPINFOW};

use super::{fallback_palette, nearest_slots, use_truecolor, BASIC_COLORS};

static LEGACY_PALETTE: std::sync::Mutex<bool> = std::sync::Mutex::new(false);

unsafe fn standard_handles() -> [(STD_HANDLE, Option<HANDLE>); 3] {
    [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE].map(|kind| {
        let handle = GetStdHandle(kind).ok().filter(|handle| {
            let mut flags = 0;
            GetHandleInformation(*handle, &mut flags).is_ok()
        });
        (kind, handle)
    })
}

unsafe fn restore_standard_handles(handles: [(STD_HANDLE, Option<HANDLE>); 3]) {
    for (kind, handle) in handles {
        if let Some(handle) = handle { let _ = SetStdHandle(kind, handle); }
    }
}

pub(super) fn attach_parent_output() -> bool {
    unsafe {
        let handles = standard_handles();
        if handles[1].1.is_some() && handles[2].1.is_some() { return true; }
        // Attachment may replace standard handles. Preserve each redirected destination,
        // including mixed redirection. Never detach or allocate on failure.
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        restore_standard_handles(handles);
        standard_handles()[1..].iter().any(|(_, handle)| handle.is_some())
    }
}

pub(super) fn spawn_console() {
    unsafe {
        if super::console_spawning_disabled() || GetConsoleWindow().0 != 0 { return; }
        let handles = standard_handles();
        if AllocConsole().is_err() { return; }
        super::OWN_CONSOLE.store(true, std::sync::atomic::Ordering::Relaxed);
        if super::stdio_attached() {
            restore_standard_handles(handles);
            return;
        }
        if let Ok(handle) = GetStdHandle(STD_OUTPUT_HANDLE) {
            let mut mode = CONSOLE_MODE(0);
            if GetConsoleMode(handle, &mut mode).is_ok() {
                let _ = SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
            }
        }
        if let Ok(handle) = GetStdHandle(STD_INPUT_HANDLE) {
            let mut mode = CONSOLE_MODE(0);
            if GetConsoleMode(handle, &mut mode).is_ok() {
                let _ = SetConsoleMode(handle, (mode & !ENABLE_QUICK_EDIT_MODE) | ENABLE_EXTENDED_FLAGS);
            }
        }
    }
}

pub(super) fn configure(target: log4rs::append::console::Target, palette: &[[u8; 3]]) -> (bool, Vec<u8>) {
    unsafe {
        let output = match target {
            log4rs::append::console::Target::Stdout => STD_OUTPUT_HANDLE,
            log4rs::append::console::Target::Stderr => STD_ERROR_HANDLE,
        };
        let Ok(handle) = GetStdHandle(output) else { return (false, nearest_slots(BASIC_COLORS, palette)) };
        let mut mode = CONSOLE_MODE(0);
        if GetConsoleMode(handle, &mut mode).is_err() { return (false, nearest_slots(BASIC_COLORS, palette)); }
        let wine = GetModuleHandleW(w!("ntdll.dll")).ok()
            .is_some_and(|module| GetProcAddress(module, s!("wine_get_version")).is_some());
        if !super::OWN_CONSOLE.load(std::sync::atomic::Ordering::Relaxed) || super::stdio_attached() {
            // Do not recreate a suppressed console or mutate a shared terminal.
            return (use_truecolor(mode.contains(ENABLE_VIRTUAL_TERMINAL_PROCESSING), wine,
                std::env::var_os("WT_SESSION").is_some()),
                nearest_slots(screen_info(handle).map_or(BASIC_COLORS, |info| info.ColorTable.map(|c| c.0)), palette));
        }
        let vt = SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING).is_ok();
        if use_truecolor(vt, wine, std::env::var_os("WT_SESSION").is_some()) {
            return (true, nearest_slots(BASIC_COLORS, palette));
        }
        // A console has one palette. Later encoders must not invalidate earlier mappings.
        let mut configured = LEGACY_PALETTE.lock().unwrap_or_else(|error| error.into_inner());
        let Some(mut info) = screen_info(handle) else { return (false, nearest_slots(BASIC_COLORS, palette)) };
        if *configured {
            return (false, nearest_slots(info.ColorTable.map(|color| color.0), palette));
        }
        *configured = true;
        let (colors, attributes) = fallback_palette(info.ColorTable.map(|color| color.0), info.wAttributes.0, palette);
        if wine {
            // Wine ignores ColorTable in SetConsoleScreenBufferInfoEx; load temporary
            // app-specific defaults while allocating a dedicated console instead.
            if info.ColorTable.map(|color| color.0) != colors {
                if let Some(_defaults) = TemporaryDefaults::install(colors, attributes) {
                    let mut input_mode = CONSOLE_MODE(0);
                    let input_mode = GetStdHandle(STD_INPUT_HANDLE).ok()
                        .filter(|input| GetConsoleMode(*input, &mut input_mode).is_ok()).map(|_| input_mode);
                    if FreeConsole().is_ok() && AllocConsole().is_ok() {
                        if let Ok(output) = GetStdHandle(output) {
                            // Wait for conhost to load its defaults before restoring the registry.
                            let _ = screen_info(output);
                            let _ = SetConsoleTextAttribute(output, CONSOLE_CHARACTER_ATTRIBUTES(attributes));
                            let _ = SetConsoleMode(output, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
                        }
                        if let (Some(mode), Ok(input)) = (input_mode, GetStdHandle(STD_INPUT_HANDLE)) {
                            let _ = SetConsoleMode(input, mode);
                        }
                    }
                }
            }
        } else {
            let window = info.srWindow;
            for (color, value) in info.ColorTable.iter_mut().zip(colors) { color.0 = value; }
            info.wAttributes.0 = attributes;
            if SetConsoleScreenBufferInfoEx(handle, &info).is_ok() {
                let _ = SetConsoleTextAttribute(handle, info.wAttributes);
                let _ = SetConsoleWindowInfo(handle, true, &window);
            }
        }
        let colors = GetStdHandle(output).ok().and_then(|handle| screen_info(handle))
            .map_or(BASIC_COLORS, |info| info.ColorTable.map(|c| c.0));
        let slots = nearest_slots(colors, palette);
        (false, slots)
    }
}

unsafe fn screen_info(handle: HANDLE) -> Option<CONSOLE_SCREEN_BUFFER_INFOEX> {
    let mut info = CONSOLE_SCREEN_BUFFER_INFOEX::default();
    info.cbSize = std::mem::size_of_val(&info) as u32;
    GetConsoleScreenBufferInfoEx(handle, &mut info).ok()?;
    Some(info)
}

fn wide(text: &str) -> Vec<u16> { text.encode_utf16().chain(Some(0)).collect() }

struct SavedValue {
    name: Vec<u16>,
    value: Option<(REG_VALUE_TYPE, Vec<u8>)>,
}

struct TemporaryDefaults {
    key: HKEY,
    path: Vec<u16>,
    created: bool,
    saved: Vec<SavedValue>,
    _lock: PaletteLock,
}

struct PaletteLock(HANDLE);

impl PaletteLock {
    unsafe fn acquire() -> Option<Self> {
        let handle = CreateMutexW(None, false, w!("Local\\UnchainedPlugin.ConsolePalette")).ok()?;
        match WaitForSingleObject(handle, 5000) {
            WAIT_OBJECT_0 | WAIT_ABANDONED => Some(Self(handle)),
            _ => { let _ = CloseHandle(handle); None }
        }
    }
}

impl Drop for PaletteLock {
    fn drop(&mut self) {
        unsafe { let _ = ReleaseMutex(self.0); let _ = CloseHandle(self.0); }
    }
}

impl TemporaryDefaults {
    unsafe fn install(colors: [u32; 16], attributes: u16) -> Option<Self> {
        let mut startup = STARTUPINFOW::default();
        GetStartupInfoW(&mut startup);
        let title = if startup.lpTitle.is_null() {
            let mut path = vec![0; 32768];
            let length = GetModuleFileNameW(None, &mut path) as usize;
            if length == 0 || length >= path.len() { return None; }
            String::from_utf16_lossy(&path[..length])
        } else { startup.lpTitle.to_string().ok()? };
        let path = wide(&format!("Console\\{}", title.replace('\\', "_")));
        Self::install_at(path, colors, attributes)
    }

    unsafe fn install_at(path: Vec<u16>, colors: [u32; 16], attributes: u16) -> Option<Self> {
        let lock = PaletteLock::acquire()?;
        let mut key = HKEY::default();
        let mut disposition = REG_CREATE_KEY_DISPOSITION::default();
        RegCreateKeyExW(HKEY_CURRENT_USER, PCWSTR(path.as_ptr()), 0, None,
            REG_OPTION_VOLATILE, KEY_QUERY_VALUE | KEY_SET_VALUE, None,
            &mut key, Some(&mut disposition)).ok()?;
        let mut guard = Self { key, path, created: disposition == REG_CREATED_NEW_KEY, saved: Vec::new(), _lock: lock };
        let values = colors.into_iter().enumerate().map(|(i, color)| (format!("ColorTable{i:02}"), color))
            .chain(Some(("ScreenColors".into(), u32::from(attributes)))).collect::<Vec<_>>();
        for (name, _) in &values {
            let name = wide(name);
            let mut kind = REG_VALUE_TYPE::default();
            let mut size = 0;
            let value = match RegQueryValueExW(key, PCWSTR(name.as_ptr()), None, Some(&mut kind), None, Some(&mut size)) {
                Ok(()) => {
                    let mut bytes = vec![0; size as usize];
                    RegQueryValueExW(key, PCWSTR(name.as_ptr()), None, Some(&mut kind),
                        Some(bytes.as_mut_ptr()), Some(&mut size)).ok()?;
                    bytes.truncate(size as usize);
                    Some((kind, bytes))
                }
                Err(error) if error.code() == HRESULT::from_win32(ERROR_FILE_NOT_FOUND.0) => None,
                Err(_) => return None,
            };
            guard.saved.push(SavedValue { name, value });
        }
        for ((_, value), saved) in values.iter().zip(&guard.saved) {
            RegSetValueExW(key, PCWSTR(saved.name.as_ptr()), 0, REG_DWORD, Some(&value.to_le_bytes())).ok()?;
        }
        Some(guard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_palettes_on_owned_and_shared_consoles() {
        use std::os::windows::process::CommandExt;
        for owned in [false, true] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--ignored", "--exact", "--nocapture",
                    &format!("{}::palette_child", module_path!().split_once("::").unwrap().1)])
                .env("UNCHAINED_TEST_OWN_CONSOLE", owned.to_string())
                .env_remove("WT_SESSION").env_remove("NO_COLOR")
                .env_remove("CLICOLOR").env_remove("CLICOLOR_FORCE")
                .creation_flags(0x08000000).output().unwrap();
            assert!(output.status.success(), "owned={owned}: {}",
                String::from_utf8_lossy(&output.stderr));
        }
    }

    #[test]
    #[ignore = "subprocess fixture for console palettes"]
    fn palette_child() {
        use std::io::Write;
        use std::os::windows::io::{AsRawHandle, BorrowedHandle};
        fn console_output() -> std::fs::File {
            let file = std::fs::OpenOptions::new().read(true).write(true).open("CONOUT$").unwrap();
            unsafe {
                SetStdHandle(STD_OUTPUT_HANDLE, HANDLE(file.as_raw_handle() as isize)).unwrap();
                SetStdHandle(STD_ERROR_HANDLE, HANDLE(file.as_raw_handle() as isize)).unwrap();
            }
            file
        }
        unsafe {
            let handles = standard_handles();
            let stderr = BorrowedHandle::borrow_raw(handles[2].1.unwrap().0 as *mut _)
                .try_clone_to_owned().unwrap();
            let stderr = std::sync::Mutex::new(std::fs::File::from(stderr));
            std::panic::set_hook(Box::new(move |info| {
                let _ = writeln!(stderr.lock().unwrap(), "{info}");
            }));
            let _ = FreeConsole();
            restore_standard_handles(handles);
            let owned = std::env::var("UNCHAINED_TEST_OWN_CONSOLE").unwrap() == "true";
            if owned { spawn_console(); }
            else { AllocConsole().unwrap(); }
            // CREATE_NO_WINDOW children retain their test-runner pipes across allocation.
            let _output = console_output();
            let before = screen_info(GetStdHandle(STD_OUTPUT_HANDLE).unwrap()).unwrap();
            let palette = [[11, 22, 33], [44, 55, 66], [77, 88, 99], [111, 122, 133], [144, 155, 166]];
            let (rgb, slots) = configure(log4rs::append::console::Target::Stdout, &palette);
            let _configured_output = console_output();
            let after = screen_info(GetStdHandle(STD_OUTPUT_HANDLE).unwrap()).unwrap();
            if !owned {
                assert_eq!(before.ColorTable.map(|c| c.0), after.ColorTable.map(|c| c.0));
                assert_eq!(before.wAttributes, after.wAttributes);
                assert_eq!(slots, nearest_slots(after.ColorTable.map(|c| c.0), &palette));
            } else if !rgb {
                // Reallocation can restore the test runner's startup pipes. Rebind
                // the console writer and verify fallback against its actual palette.
                let (_, slots) = configure(log4rs::append::console::Target::Stdout, &palette);
                assert_eq!(slots, nearest_slots(after.ColorTable.map(|c| c.0), &palette));
                let other = [[10, 30, 50]; 5];
                let (_, slots) = configure(log4rs::append::console::Target::Stderr, &other);
                let current = screen_info(GetStdHandle(STD_ERROR_HANDLE).unwrap()).unwrap();
                assert_eq!(current.ColorTable.map(|c| c.0), after.ColorTable.map(|c| c.0));
                assert_eq!(slots, nearest_slots(current.ColorTable.map(|c| c.0), &other));
            }
            let _ = FreeConsole();
            restore_standard_handles(handles);
        }
    }

    #[test]
    fn attachment_and_console_spawning_are_independent() {
        for attach in [false, true] {
            for no_spawn in [false, true] {
                use std::os::windows::process::CommandExt;
                let output = std::process::Command::new(std::env::current_exe().unwrap())
                    .args(["--ignored", "--exact", "--nocapture",
                        &format!("{}::console_flags_child", module_path!().split_once("::").unwrap().1)])
                    .env("UNCHAINED_TEST_ATTACH", attach.to_string())
                    .env("UNCHAINED_TEST_NO_SPAWN", no_spawn.to_string())
                    .creation_flags(0x08000000).output().unwrap();
                assert!(output.status.success(), "attach={attach}, no_spawn={no_spawn}: {}",
                    String::from_utf8_lossy(&output.stderr));
                if attach {
                    assert!(String::from_utf8_lossy(&output.stdout).contains("inherited stdout preserved"));
                    assert!(String::from_utf8_lossy(&output.stderr).contains("inherited stderr preserved"));
                }
            }
        }
    }

    #[test]
    #[ignore = "subprocess fixture for independent console flags"]
    fn console_flags_child() {
        use std::io::Write;
        use std::os::windows::io::BorrowedHandle;
        unsafe {
            let handles = standard_handles();
            let _ = FreeConsole();
            restore_standard_handles(handles);
            assert_eq!(GetConsoleWindow().0, 0);
            let attach = std::env::var("UNCHAINED_TEST_ATTACH").unwrap() == "true";
            let no_spawn = std::env::var("UNCHAINED_TEST_NO_SPAWN").unwrap() == "true";
            if no_spawn { super::super::disable_console_spawning(); }
            if attach { super::super::attach_stdio(); }
            assert_eq!(super::super::console_spawning_disabled(), no_spawn);
            super::super::spawn_console();
            assert_eq!(GetConsoleWindow().0 != 0, !no_spawn);
            if attach {
                assert_eq!(standard_handles(), handles);
                configure(log4rs::append::console::Target::Stdout, &[[242, 139, 130]]);
                configure(log4rs::append::console::Target::Stderr, &[[242, 139, 130]]);
                assert_eq!(standard_handles(), handles);
                for (kind, text) in [(STD_OUTPUT_HANDLE, "inherited stdout preserved\n"),
                    (STD_ERROR_HANDLE, "inherited stderr preserved\n")] {
                    let handle = GetStdHandle(kind).unwrap();
                    let owned = BorrowedHandle::borrow_raw(handle.0 as *mut _).try_clone_to_owned().unwrap();
                    std::fs::File::from(owned).write_all(text.as_bytes()).unwrap();
                }
            }
        }
    }

    #[test]
    fn temporary_defaults_remove_new_key_and_restore_existing_values() {
        unsafe {
            let path = wide(&format!("Console\\UnchainedPluginPaletteTest-{}", std::process::id()));
            let colors = [0x123456; 16];
            drop(TemporaryDefaults::install_at(path.clone(), colors, 7).unwrap());
            let mut key = HKEY::default();
            assert!(RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(path.as_ptr()), 0, KEY_READ, &mut key).is_err());

            RegCreateKeyExW(HKEY_CURRENT_USER, PCWSTR(path.as_ptr()), 0, None,
                REG_OPTION_VOLATILE, KEY_ALL_ACCESS, None, &mut key, None).unwrap();
            let name = wide("ColorTable07");
            let original = [1, 2, 3, 4];
            RegSetValueExW(key, PCWSTR(name.as_ptr()), 0, REG_BINARY, Some(&original)).unwrap();
            {
                let _guard = TemporaryDefaults::install_at(path.clone(), colors, 15).unwrap();
            }
            let mut kind = REG_VALUE_TYPE::default();
            let mut bytes = [0; 4];
            let mut size = 4;
            RegQueryValueExW(key, PCWSTR(name.as_ptr()), None, Some(&mut kind), Some(bytes.as_mut_ptr()), Some(&mut size)).unwrap();
            assert_eq!(kind, REG_BINARY);
            assert_eq!(bytes, original);
            assert!(RegQueryValueExW(key, w!("ColorTable08"), None, None, None, None).is_err());
            RegCloseKey(key).unwrap();
            RegDeleteKeyW(HKEY_CURRENT_USER, PCWSTR(path.as_ptr())).unwrap();
        }
    }
}

impl Drop for TemporaryDefaults {
    fn drop(&mut self) {
        unsafe {
            for saved in &self.saved {
                match &saved.value {
                    Some((kind, bytes)) => { let _ = RegSetValueExW(self.key, PCWSTR(saved.name.as_ptr()), 0, *kind, Some(bytes)); }
                    None => { let _ = RegDeleteValueW(self.key, PCWSTR(saved.name.as_ptr())); }
                }
            }
            let _ = RegCloseKey(self.key);
            if self.created { let _ = RegDeleteKeyW(HKEY_CURRENT_USER, PCWSTR(self.path.as_ptr())); }
        }
    }
}
