use super::{FString, TArray};

/// FURL from Engine/EngineBaseTypes.h (UE 4.25Plus).
#[repr(C)]
pub struct FURL {
    pub protocol: FString,
    pub host: FString,
    pub port: i32,
    pub valid: i32,
    pub map: FString,
    pub redirect_url: FString,
    pub op: TArray<FString>,
    pub portal: FString,
}

const _: () = {
    assert!(std::mem::size_of::<FURL>() == 0x68);
    assert!(std::mem::offset_of!(FURL, map) == 0x28);
};
