//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/GameFramework/PlayerState.h>.

#![allow(non_snake_case)]
use bitflags::bitflags;

bitflags! {
    // Note: Use 'Default' or manual names for pretty printing
    #[derive(Debug, Clone, Copy)]
    pub struct PlayerFlags: u8 {
        const SHOULD_UPDATE_PING = 1 << 0;
        const IS_SPECTATOR       = 1 << 1;
        const ONLY_SPECTATOR     = 1 << 2;
        const IS_A_BOT           = 1 << 3;
        const IS_INACTIVE        = 1 << 4;
        const FROM_PREVIOUS      = 1 << 5;
    }
}
