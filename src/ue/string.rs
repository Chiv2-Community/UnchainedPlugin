use super::TArray;
use windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use windows::Win32::System::Threading::GetCurrentProcess;

pub type FString = TArray<u16>;

fn trim_trailing_nuls(slice: &[u16]) -> &[u16] {
    let last_non_nul = slice.iter().rposition(|&code_unit| code_unit != 0);
    match last_non_nul {
        Some(index) => &slice[..=index],
        None => &[],
    }
}

#[derive(Debug, Clone, Copy)]
pub enum FStringCopyError {
    NullOrEmptyBuffer,
    LengthOverflow,
    MemoryReadFailed,
    PartialRead,
    Utf16DecodeFailed,
}

impl FStringCopyError {
    pub fn as_str(self) -> &'static str {
        match self {
            FStringCopyError::NullOrEmptyBuffer => "FString had null or empty backing buffer",
            FStringCopyError::LengthOverflow => "FString backing length overflowed byte conversion",
            FStringCopyError::MemoryReadFailed => "FString backing memory copy failed",
            FStringCopyError::PartialRead => "FString backing memory copy was partial",
            FStringCopyError::Utf16DecodeFailed => "FString UTF-16 decode failed",
        }
    }
}

impl TArray<u16> {
    /// Using direct to_string on TArray<u16> (or the FString type) binds a reference to a string
    /// in the FString's backing buffer, which is not safe to do in a multithreaded environment.
    /// This function copies the buffer to a new string, which is safe to do in a multithreaded
    /// environment.
    pub fn copy_to_string(&self) -> Result<String, FStringCopyError> {
        let utf16_len = self.len();
        let utf16_ptr = self.as_ptr();
        if utf16_len == 0 || utf16_ptr.is_null() {
            return Err(FStringCopyError::NullOrEmptyBuffer);
        }

        let byte_len = utf16_len
            .checked_mul(std::mem::size_of::<u16>())
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

        let trimmed_utf16 = trim_trailing_nuls(&utf16);
        String::from_utf16(trimmed_utf16).map_err(|_| FStringCopyError::Utf16DecodeFailed)
    }
}

impl From<&str> for FString {
    fn from(value: &str) -> Self {
        let buffer: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
        Self::from(buffer.as_slice())
    }
}

impl std::fmt::Display for FString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let slice = trim_trailing_nuls(self.as_slice());
        write!(f, "{}", String::from_utf16_lossy(slice))
    }
}
