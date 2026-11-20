//! Native source: <https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/EKillReason.h>.
//! The original native header owner is unresolved; retain this location pending evidence.

#![allow(non_snake_case)]
use strum::Display;

#[unchained_macros::runtime_bindings]
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display)]
pub enum EKillReason {
    Damage = 0,
    FallDamage = 1,
    Suicide = 2,
    TapOut = 3,
    OutOfCombat = 4,
    FellOutOfWorld = 5,
    Disconnect = 6,
    ForwardSpawn = 7,
    SwitchedTeams = 8,
    Spectator = 9,
}

impl EKillReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            EKillReason::Damage => "Damage",
            EKillReason::FallDamage => "FallDamage",
            EKillReason::Suicide => "Suicide",
            EKillReason::TapOut => "TapOut",
            EKillReason::OutOfCombat => "OutOfCombat",
            EKillReason::FellOutOfWorld => "FellOutOfWorld",
            EKillReason::Disconnect => "Disconnect",
            EKillReason::ForwardSpawn => "ForwardSpawn",
            EKillReason::SwitchedTeams => "SwitchedTeams",
            EKillReason::Spectator => "Spectator",
        }
    }
}

impl TryFrom<u8> for EKillReason {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(EKillReason::Damage),
            1 => Ok(EKillReason::FallDamage),
            2 => Ok(EKillReason::Suicide),
            3 => Ok(EKillReason::TapOut),
            4 => Ok(EKillReason::OutOfCombat),
            5 => Ok(EKillReason::FellOutOfWorld),
            6 => Ok(EKillReason::Disconnect),
            7 => Ok(EKillReason::ForwardSpawn),
            8 => Ok(EKillReason::SwitchedTeams),
            9 => Ok(EKillReason::Spectator),
            _ => Err(()),
        }
    }
}
