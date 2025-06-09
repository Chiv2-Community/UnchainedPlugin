

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

use std::str::FromStr;

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
