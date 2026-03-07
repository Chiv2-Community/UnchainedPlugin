use super::TArray;

pub type FString = TArray<u16>;

fn trim_trailing_nuls(slice: &[u16]) -> &[u16] {
    let last_non_nul = slice.iter().rposition(|&code_unit| code_unit != 0);
    match last_non_nul {
        Some(index) => &slice[..=index],
        None => &[],
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