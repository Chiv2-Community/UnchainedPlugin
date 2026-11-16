//! EOS Connect identity-only tokens through the public SDK ABI.
//! References: EOS_Connect_CopyIdTokenOptions and EOS_Connect_IdToken.
//! Platform handles are captured at creation and removed before native release.
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use std::{
    ffi::{c_char, c_void, OsString},
    os::windows::ffi::{OsStrExt, OsStringExt},
};
use windows::{
    core::{s, PCSTR, PCWSTR},
    Win32::{
        Foundation::HMODULE,
        System::LibraryLoader::{
            GetModuleFileNameW, GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
        },
    },
};

type Handle = *mut c_void;
type GetConnect = unsafe extern "C" fn(Handle) -> Handle;
type LoggedInCount = unsafe extern "C" fn(Handle) -> i32;
type LoggedInUser = unsafe extern "C" fn(Handle, i32) -> Handle;
type CopyToken = unsafe extern "C" fn(Handle, *const CopyOptions, *mut *mut IdToken) -> i32;
type ReleaseToken = unsafe extern "C" fn(*mut IdToken);

#[repr(C)]
struct CopyOptions {
    api_version: i32,
    product_user_id: Handle,
}

#[repr(C)]
struct IdToken {
    api_version: i32,
    product_user_id: Handle,
    json_web_token: *const c_char,
}

struct Api {
    get_connect: GetConnect,
    user_count: LoggedInCount,
    user: LoggedInUser,
    copy_token: CopyToken,
    release_token: ReleaseToken,
}

static API: OnceCell<Api> = OnceCell::new();
static PLATFORMS: Mutex<Vec<usize>> = Mutex::new(Vec::new());

retour::static_detour! {
    static CREATE: unsafe extern "C" fn(*const c_void) -> Handle;
    static RELEASE: unsafe extern "C" fn(Handle);
}

unsafe extern "C" fn created(options: *const c_void) -> Handle {
    let handle = unsafe { CREATE.call(options) };
    if !handle.is_null() {
        let mut platforms = PLATFORMS.lock();
        if platforms.len() < 16 && !platforms.contains(&(handle as usize)) {
            platforms.push(handle as usize);
        }
    }
    handle
}

unsafe extern "C" fn released(handle: Handle) {
    // Copying a token holds this lock, so release cannot invalidate an in-use SDK
    // handle. Drop it before native release, which may invoke other callbacks.
    PLATFORMS.lock().retain(|known| *known != handle as usize);
    unsafe { RELEASE.call(handle) };
}

unsafe fn export(module: HMODULE, name: PCSTR) -> Result<usize, &'static str> {
    unsafe { GetProcAddress(module, name) }
        .map(|address| address as usize)
        .ok_or("Required EOS identity API unavailable")
}

/// Install before the game's first EOS platform creation. No login or credentials
/// are changed; late injection without a captured platform fails closed.
pub unsafe fn install() -> Result<(), Box<dyn std::error::Error>> {
    // Redpoint delay-loads the SDK. Load its exact game-relative path before
    // platform creation, retaining our module reference for process lifetime.
    let mut path = vec![0u16; 32768];
    let length = unsafe { GetModuleFileNameW(None, &mut path) } as usize;
    if length == 0 || length >= path.len() {
        return Err("Game executable path unavailable".into());
    }
    let path = std::path::PathBuf::from(OsString::from_wide(&path[..length]));
    let path = path
        .parent()
        .ok_or("Game directory unavailable")?
        .join("RedpointEOS")
        .join("EOSSDK-Win64-Shipping.dll");
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let module = unsafe {
        LoadLibraryExW(
            PCWSTR(path.as_ptr()),
            None,
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
        )
    }?;
    let api = Api {
        get_connect: unsafe {
            std::mem::transmute::<usize, GetConnect>(export(
                module,
                s!("EOS_Platform_GetConnectInterface"),
            )?)
        },
        user_count: unsafe {
            std::mem::transmute::<usize, LoggedInCount>(export(
                module,
                s!("EOS_Connect_GetLoggedInUsersCount"),
            )?)
        },
        user: unsafe {
            std::mem::transmute::<usize, LoggedInUser>(export(
                module,
                s!("EOS_Connect_GetLoggedInUserByIndex"),
            )?)
        },
        copy_token: unsafe {
            std::mem::transmute::<usize, CopyToken>(export(module, s!("EOS_Connect_CopyIdToken"))?)
        },
        release_token: unsafe {
            std::mem::transmute::<usize, ReleaseToken>(export(
                module,
                s!("EOS_Connect_IdToken_Release"),
            )?)
        },
    };
    let create: unsafe extern "C" fn(*const c_void) -> Handle =
        unsafe { std::mem::transmute(export(module, s!("EOS_Platform_Create"))?) };
    let release: unsafe extern "C" fn(Handle) =
        unsafe { std::mem::transmute(export(module, s!("EOS_Platform_Release"))?) };
    API.set(api)
        .map_err(|_| "EOS identity adapter already installed")?;
    unsafe {
        CREATE.initialize(create, |options| created(options))?;
        RELEASE.initialize(release, |handle| released(handle))?;
        RELEASE.enable()?;
        if let Err(error) = CREATE.enable() {
            let _ = RELEASE.disable();
            return Err(error.into());
        }
    }
    Ok(())
}

struct OwnedToken<'a> {
    token: *mut IdToken,
    api: &'a Api,
}

impl Drop for OwnedToken<'_> {
    fn drop(&mut self) {
        if !self.token.is_null() {
            unsafe { (self.api.release_token)(self.token) };
        }
    }
}

/// Called on the game thread. Returns only an unambiguous single local user's
/// identity token; the backend must verify every claim before using it.
pub unsafe fn copy_local_id_token() -> Option<String> {
    let api = API.get()?;
    if !CREATE.is_enabled() || !RELEASE.is_enabled() {
        return None;
    }
    let platforms = PLATFORMS.lock();
    let mut result = None;
    for platform in platforms.iter().copied() {
        let connect = unsafe { (api.get_connect)(platform as Handle) };
        if connect.is_null() {
            continue;
        }
        let count = unsafe { (api.user_count)(connect) };
        if count == 0 {
            continue;
        }
        if count != 1 || result.is_some() {
            return None;
        }
        let user = unsafe { (api.user)(connect, 0) };
        if user.is_null() {
            return None;
        }
        let options = CopyOptions {
            api_version: 1,
            product_user_id: user,
        };
        let mut token = OwnedToken {
            token: std::ptr::null_mut(),
            api,
        };
        if unsafe { (api.copy_token)(connect, &options, &mut token.token) } != 0 {
            return None;
        }
        let native = unsafe { token.token.as_ref()? };
        if native.api_version != 1
            || native.product_user_id != user
            || native.json_web_token.is_null()
        {
            return None;
        }
        // The SDK owns a NUL-terminated buffer; bound the copy and never log it.
        let mut bytes = Vec::new();
        for index in 0..=16384 {
            let byte = unsafe { native.json_web_token.add(index).read() } as u8;
            if byte == 0 {
                break;
            }
            if index == 16384 {
                return None;
            }
            bytes.push(byte);
        }
        let value = String::from_utf8(bytes).ok()?;
        if value.is_empty() {
            return None;
        }
        result = Some(value);
    }
    result
}
