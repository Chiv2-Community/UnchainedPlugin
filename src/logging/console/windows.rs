use windows::core::{s, w, HRESULT, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, GetHandleInformation, ERROR_FILE_NOT_FOUND, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0};
use windows::Win32::System::Console::*;
use windows::Win32::System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Registry::*;
use windows::Win32::System::Threading::{CreateMutexW, GetStartupInfoW, ReleaseMutex, WaitForSingleObject, STARTUPINFOW};

use super::{fallback_palette, use_truecolor};

pub(super) fn attach_parent_output() -> bool {
    unsafe {
        let valid = |kind| GetStdHandle(kind).ok().filter(|handle| {
            let mut flags = 0;
            GetHandleInformation(*handle, &mut flags).is_ok()
        });
        let stdout = valid(STD_OUTPUT_HANDLE);
        let stderr = valid(STD_ERROR_HANDLE);
        if stdout.is_some() && stderr.is_some() { return true; }
        // Attachment may replace standard handles. Preserve each redirected destination,
        // including mixed redirection. Never detach or allocate on failure.
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        if let Some(handle) = stdout { let _ = SetStdHandle(STD_OUTPUT_HANDLE, handle); }
        if let Some(handle) = stderr { let _ = SetStdHandle(STD_ERROR_HANDLE, handle); }
        valid(STD_OUTPUT_HANDLE).is_some() || valid(STD_ERROR_HANDLE).is_some()
    }
}

pub(super) fn configure(target: log4rs::append::console::Target) -> bool {
    unsafe {
        let output = match target {
            log4rs::append::console::Target::Stdout => STD_OUTPUT_HANDLE,
            log4rs::append::console::Target::Stderr => STD_ERROR_HANDLE,
        };
        let Ok(handle) = GetStdHandle(output) else { return false };
        let mut mode = CONSOLE_MODE(0);
        if GetConsoleMode(handle, &mut mode).is_err() { return false; }
        let wine = GetModuleHandleW(w!("ntdll.dll")).ok()
            .is_some_and(|module| GetProcAddress(module, s!("wine_get_version")).is_some());
        if super::console_spawning_disabled() {
            // Do not recreate a suppressed console or mutate a shared terminal.
            return use_truecolor(mode.contains(ENABLE_VIRTUAL_TERMINAL_PROCESSING), wine,
                std::env::var_os("WT_SESSION").is_some());
        }
        let vt = SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING).is_ok();
        if use_truecolor(vt, wine, std::env::var_os("WT_SESSION").is_some()) { return true; }
        let Some(mut info) = screen_info(handle) else { return false };
        let (colors, attributes) = fallback_palette(info.ColorTable.map(|color| color.0), info.wAttributes.0);
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
        false
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
