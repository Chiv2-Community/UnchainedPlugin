#![allow(non_snake_case)]
use std::str::FromStr;

#[repr(C)]
#[derive(Debug)]
pub struct ATBLPlayerController { 
    _private: [u8; 0x1348],
	pub bOnlineInventoryInitialized: bool,
	pub bPlayerCustomizationReceived: bool,
}

// Chat type enum
// FIXME: More compact, wtf is this
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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