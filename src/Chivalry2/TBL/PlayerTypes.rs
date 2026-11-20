//! Native source: TBL/Source/TBL/PlayerTypes.h.

#![allow(non_snake_case)]
use std::str::FromStr;
use strum::Display;

// Chat type enum
// FIXME: More compact, wtf is this
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
#[allow(clippy::upper_case_acronyms)]
pub enum EChatType {
    AllSay,
    TeamSay,
    Whisper,
    Admin,
    Objective,
    System,
    ServerSay,
    Debug,
    CrosshairMsg,
    Backend,
    Party,
    Spectator,
    ClosedCaption,
    ClosedCaptionMason,
    ClosedCaptionAgatha,
    MAX,
}

impl EChatType {
    // A manual list of all variants for the loop
    pub const ALL: [EChatType; 15] = [
        Self::AllSay,
        Self::TeamSay,
        Self::Whisper,
        Self::Admin,
        Self::Objective,
        Self::System,
        Self::ServerSay,
        Self::Debug,
        Self::CrosshairMsg,
        Self::Backend,
        Self::Party,
        Self::Spectator,
        Self::ClosedCaption,
        Self::ClosedCaptionMason,
        Self::ClosedCaptionAgatha,
    ];

    // Manual string conversion
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AllSay => "AllSay",
            Self::TeamSay => "TeamSay",
            Self::Whisper => "Whisper",
            Self::Admin => "Admin",
            Self::Objective => "Objective",
            Self::System => "System",
            Self::ServerSay => "ServerSay",
            Self::Debug => "Debug",
            Self::CrosshairMsg => "CrosshairMsg",
            Self::Backend => "Backend",
            Self::Party => "Party",
            Self::Spectator => "Spectator",
            Self::ClosedCaption => "ClosedCaption",
            Self::ClosedCaptionMason => "ClosedCaptionMason",
            Self::ClosedCaptionAgatha => "ClosedCaptionAgatha",
            Self::MAX => "MAX",
        }
    }
}

impl FromStr for EChatType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "AllSay" => Ok(EChatType::AllSay),
            "TeamSay" => Ok(EChatType::TeamSay),
            "Whisper" => Ok(EChatType::Whisper),
            "Admin" => Ok(EChatType::Admin),
            "Objective" => Ok(EChatType::Objective),
            "System" => Ok(EChatType::System),
            "ServerSay" => Ok(EChatType::ServerSay),
            "Debug" => Ok(EChatType::Debug),
            "CrosshairMsg" => Ok(EChatType::CrosshairMsg),
            "Backend" => Ok(EChatType::Backend),
            "Party" => Ok(EChatType::Party),
            "Spectator" => Ok(EChatType::Spectator),
            "ClosedCaption" => Ok(EChatType::ClosedCaption),
            "ClosedCaptionMason" => Ok(EChatType::ClosedCaptionMason),
            "ClosedCaptionAgatha" => Ok(EChatType::ClosedCaptionAgatha),
            "MAX" => Ok(EChatType::MAX),
            _ => Err(()),
        }
    }
}

impl TryFrom<u8> for EChatType {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(EChatType::AllSay),
            1 => Ok(EChatType::TeamSay),
            2 => Ok(EChatType::Whisper),
            3 => Ok(EChatType::Admin),
            4 => Ok(EChatType::Objective),
            5 => Ok(EChatType::System),
            6 => Ok(EChatType::ServerSay),
            7 => Ok(EChatType::Debug),
            8 => Ok(EChatType::CrosshairMsg),
            9 => Ok(EChatType::Backend),
            10 => Ok(EChatType::Party),
            11 => Ok(EChatType::Spectator),
            12 => Ok(EChatType::ClosedCaption),
            13 => Ok(EChatType::ClosedCaptionMason),
            14 => Ok(EChatType::ClosedCaptionAgatha),
            15 => Ok(EChatType::MAX),
            _ => Err(()),
        }
    }
}

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
