//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/GameFramework/PlayerState.h>.

use crate::Chivalry2::Engine::APawn;
use crate::Chivalry2::Engine::FUniqueNetIdRepl;

use crate::Chivalry2::Engine::FString;
use crate::Chivalry2::Engine::UObjectType;

use bitflags::bitflags;

use std::ffi::c_void;

use unchained_macros::bind_resolved_func;

define_pattern_resolver!(
    APlayerState__GetPlayerName,
    Call,
    ["48 8D 55 E7 48 8B CB E8 | ?? ?? ?? ?? 83 78 08 00 74"]
);

bitflags! {
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

// SAFETY: This native layout has its UObject prefix at offset zero.
unsafe impl UObjectType for APlayerState {
    const CLASS_NAME: &'static str = "PlayerState";
}

#[rust_ctables::cpp_struct]
pub struct APlayerState {
    #[inherit]
    pub base: crate::Chivalry2::Engine::AInfo,

    pub score: f32,     // 0x0258
    pub player_id: i32, // 0x025C
    pub ping: u8,       // 0x0260
    pub _padding_0: u8, // 0x0261

    // Bitfields/Flags
    // pub player_flags: u8,               // 0x0262 (Spectator, Bot, Inactive, etc.)
    pub player_flags: PlayerFlags,
    pub _padding_1: u8, // 0x0263

    pub start_time: i32,                   // 0x0264
    pub engine_message_class: *mut c_void, // 0x0268 (ULocalMessage*)
    pub _padding_2: [u8; 0x8],             // 0x0270
    pub saved_network_address: FString,    // 0x0278
    pub unique_id: FUniqueNetIdRepl,       // 0x0288 (Size: 0x28)
    pub _padding_3: [u8; 0x8],             // 0x02B0
    pub pawn_private: *mut APawn,          // 0x02B8
    pub _padding_4: [u8; 0x78],            // 0x02C0
    pub player_name_private: FString,      // 0x0338
    pub _padding_last: [u8; 0x10],
}

impl APlayerState {
    /// # Safety
    /// UObjectType::from_ptr's game-thread and lifetime requirements apply.
    pub(crate) unsafe fn from_field<'a, P>(ptr: *const P, via: &str) -> Option<&'a Self> {
        if ptr.is_null() {
            return None;
        }
        let state = Self::from_ptr(ptr);
        if state.is_none() {
            swarn!(
                "ignoring PlayerState {:p} from {}: not a live PlayerState",
                ptr,
                via
            );
        }
        state
    }

    pub fn is_bot(&self) -> bool {
        self.player_flags.contains(PlayerFlags::IS_A_BOT)
    }

    /// # Safety
    /// UObjectType::from_ptr's game-thread and lifetime requirements apply.
    pub unsafe fn pawn(&self) -> Option<&APawn> {
        APawn::from_ptr(self.pawn_private)
    }

    /// Call native GetPlayerName, including its custom-name handling.
    ///
    /// # Safety
    /// This must be a live native player state, and the native callable must be initialized.
    pub unsafe fn get_player_name(&self) -> String {
        let mut name = FString::new();
        (self as *const Self).GetPlayerName(&mut name);
        name.to_string()
    }

    /// Native APlayerState::GetPlayerName, including custom-name handling.
    ///
    /// # Safety
    /// Self must be live on the engine thread; output must be writable native
    /// FString return storage, whose result the caller owns.
    #[bind_resolved_func(self::APlayerState__GetPlayerName)]
    pub unsafe fn GetPlayerName<'a>(
        self: *const Self,
        output: &'a mut FString,
    ) -> &'a mut FString {
    }
}
