//! Native source: <https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/EFaction.h>.

#[unchained_macros_core::runtime_bindings]
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EFaction {
    Agatha = 0,
    Mason = 1,
    FFA = 2,
    None = 3,
    All = 4,
    Tenosia = 5,
    MAX = 6,
}

impl EFaction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Agatha => "Agatha",
            Self::Mason => "Mason",
            Self::FFA => "FFA",
            Self::None => "None",
            Self::All => "All",
            Self::Tenosia => "Tenosia",
            Self::MAX => "MAX",
        }
    }
}
