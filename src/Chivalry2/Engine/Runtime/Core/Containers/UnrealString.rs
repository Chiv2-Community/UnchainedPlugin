//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/Containers/UnrealString.h>.

#![allow(private_interfaces)]
use crate::Chivalry2::Engine::TArray;
use crate::Chivalry2::Engine::TCHAR;
use ::windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use ::windows::Win32::System::Threading::GetCurrentProcess;

define_pattern_resolver!(FString__AppendChars, [
    "45 85 C0 0F 84 89 00 00 00 48 89 5C 24 18 48 89 6C 24 20 56 48 83 EC 20 48 89 7C 24 30 48 8B EA 48 63 79 08 48 8B D9 4C 89 74 24 38 45 33 F6 85 FF 49 63 F0 41 8B C6 0F 94 C0 03 C7 03 C6 89 41 08 3B 41 0C 7E 07 8B D7 E8 ?? ?? ?? ?? 85 FF 49 8B C6 48 8B CF 48 8B D5 0F 95 C0 48 2B C8 48 8B 03 48 8D 1C 36 4C 8B C3 48 8D 3C 48 48 8B CF E8 ?? ?? ?? ?? 48 8B 6C 24 48 66 44 89 34 3B 4C 8B 74 24 38 48 8B 7C 24 30 48 8B 5C 24 40 48 83 C4 20 5E C3" // Universal
]);

define_pattern_resolver!(FString__Find, [
    "48 89 5C 24 10 48 89 74 24 18 48 89 7C 24 20 41 54 41 56 41 57 48 83 EC 50 45 8B E1 48 8B FA 4C 8B F1"
]);

define_pattern_resolver!(
    FString__ParseIntoArray,
    ["44 88 4C 24 20 53 55 56 57 41 55 41 57 48 83 EC 28 48 8B FA 48 8B D9"]
);

#[unchained_macros::runtime_bindings]
pub type FString = TArray<TCHAR>;

#[unchained_macros::runtime_bindings]
impl TArray<TCHAR> {
    fn trim_trailing_nuls(slice: &[TCHAR]) -> &[TCHAR] {
        let last_non_nul = slice.iter().rposition(|&code_unit| code_unit != 0);
        match last_non_nul {
            Some(index) => &slice[..=index],
            None => &[],
        }
    }

    /// # Safety
    /// Self must be a live writable native string with exclusive access. Text
    /// must contain count readable UTF-16 units; count must be nonnegative.
    #[unchained_macros::bind_resolved_func(self::FString__AppendChars)]
    pub unsafe fn AppendChars(self: *mut Self, text: *const TCHAR, count: i32) {}

    /// Using direct to_string on TArray<TCHAR> (or the FString type) binds a reference to a string
    /// in the FString's backing buffer, which is not safe to do in a multithreaded environment.
    /// This function copies the buffer to a new string, which is safe to do in a multithreaded
    /// environment.
    ///
    /// The header (data pointer, length, capacity) is read the same way as the buffer, so an
    /// FString inside a dangling or garbage object returns an error instead of raising an access
    /// violation. A header whose length is negative, exceeds its capacity or is implausibly large
    /// is rejected before anything is allocated.
    pub fn copy_to_string(&self) -> Result<String, FStringCopyError> {
        #[derive(Clone, Copy)]
        #[repr(C)]
        struct Header {
            data: *const TCHAR,
            num: i32,
            max: i32,
        }
        const MAX_CODE_UNITS: i32 = 1 << 20;

        let header =
            crate::Chivalry2::Engine::FMemory::try_read(self as *const Self as *const Header)
                .ok_or(FStringCopyError::MemoryReadFailed)?;
        if header.num == 0 || header.data.is_null() {
            return Err(FStringCopyError::NullOrEmptyBuffer);
        }
        if header.num < 0 || header.num > header.max || header.num > MAX_CODE_UNITS {
            return Err(FStringCopyError::ImplausibleHeader);
        }
        let utf16_len = header.num as usize;
        let utf16_ptr = header.data;

        let byte_len = utf16_len
            .checked_mul(std::mem::size_of::<TCHAR>())
            .ok_or(FStringCopyError::LengthOverflow)?;
        let mut utf16 = vec![0u16; utf16_len];
        let mut bytes_read = 0usize;

        let read_result = unsafe {
            ReadProcessMemory(
                GetCurrentProcess(),
                utf16_ptr.cast(),
                utf16.as_mut_ptr().cast(),
                byte_len,
                Some(&mut bytes_read),
            )
        };

        if read_result.is_err() {
            return Err(FStringCopyError::MemoryReadFailed);
        }

        if bytes_read != byte_len {
            return Err(FStringCopyError::PartialRead);
        }

        let trimmed_utf16 = FString::trim_trailing_nuls(&utf16);
        String::from_utf16(trimmed_utf16).map_err(|_| FStringCopyError::Utf16DecodeFailed)
    }

    #[unchained_macros::bind_resolved_func(FString__Find)]
    pub(crate) unsafe fn Find(
        self: *const Self,
        needle: *const TCHAR,
        search_case: i32,
        search_direction: i32,
        start_position: i32,
    ) -> i32 {
    }

    #[unchained_macros::bind_resolved_func(FString__ParseIntoArray)]
    pub(crate) unsafe fn ParseIntoArray(
        self: *const Self,
        output: *mut TArray<FString>,
        delimiter: *const TCHAR,
        cull_empty: bool,
    ) -> i32 {
    }
}

#[derive(Debug, Clone, Copy)]
pub enum FStringCopyError {
    NullOrEmptyBuffer,
    LengthOverflow,
    MemoryReadFailed,
    PartialRead,
    Utf16DecodeFailed,
    ImplausibleHeader,
}

impl FStringCopyError {
    pub fn as_str(self) -> &'static str {
        match self {
            FStringCopyError::NullOrEmptyBuffer => "FString had null or empty backing buffer",
            FStringCopyError::LengthOverflow => "FString backing length overflowed byte conversion",
            FStringCopyError::MemoryReadFailed => "FString backing memory copy failed",
            FStringCopyError::PartialRead => "FString backing memory copy was partial",
            FStringCopyError::Utf16DecodeFailed => "FString UTF-16 decode failed",
            FStringCopyError::ImplausibleHeader => "FString header had an impossible length",
        }
    }
}

impl From<&str> for FString {
    fn from(value: &str) -> Self {
        let buffer: Vec<TCHAR> = value.encode_utf16().chain(std::iter::once(0)).collect();
        Self::from(buffer.as_slice())
    }
}

impl std::fmt::Display for FString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let slice = FString::trim_trailing_nuls(self.as_slice());
        write!(f, "{}", String::from_utf16_lossy(slice))
    }
}
