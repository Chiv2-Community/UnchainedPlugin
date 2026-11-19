//! ArgonSDK main: [ATBLGameMode](https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/TBLGameMode.h).

use super::{ATBLGameMode__Logout, ATBLGameMode__PreLogin};
use crate::chivalry2::engine::AController;
use crate::chivalry2::engine::FString;
use crate::chivalry2::engine::FUniqueNetIdRepl;
use crate::chivalry2::tbl::ATBLGameMode;

/* seems to be broken, or just incorrect.
       Looks right in the binary, but keeps matching the wrong func
    XrefFirst,
    [patternsleuth::resolvers::unreal::util::utf8_pattern(
        " Minutes"
    )]
);
*/

#[unchained_macros::runtime_bindings]
impl ATBLGameMode {
    #[unchained_macros::bind_resolved_func(ATBLGameMode__PreLogin)]
    pub(crate) unsafe fn pre_login(
        self: *mut Self,
        options: *const FString,
        address: *const FString,
        unique_id: *const FUniqueNetIdRepl,
        error_message: *mut FString,
    ) {
    }

    #[unchained_macros::bind_resolved_func(ATBLGameMode__Logout)]
    pub(crate) unsafe fn logout(self: *mut Self, exiting_player: *mut AController) {}
}

// Disable stat increase to prevent stutters in offline play (and server mode?)
// bool __thiscall FTBLOnlineStats::IncStat(FTBLOnlineStats *this,int param_1,int param_2)
// define_pattern_resolver!(IncStat, [
//     "48 89 5C 24 10 48 89 6C 24 18 48 89 74 24 20 57 48 83 EC 20 80 79 10 00 41 8B E8 48 63 FA 48 8B D9 ?? ?? ?? ?? ?? ?? 48 8B 05 52 86 1F 04 80 3C" // Universal
// ]);
// CREATE_HOOK!(IncStat, ACTIVE, NONE, (),
//     (this_ptr: *mut FTBLOnlineStats, p1: i32, p2: i32),
// {
//     // sinfo!("Stat: {:?}", p1);
//     // CALL_ORIGINAL!(IncStat(this_ptr, p1, p2));
// });

// Disable stat change to prevent stutters in offline play (and server mode?)
// Nihi: Other OnlineStats-related functions could also impair performance. Needs some investigation. Alternatively, look into what exactly causes this (network connection?)
// bool __thiscall FTBLOnlineStats::SetStatInner(FTBLOnlineStats *this,int param_1,int param_2)
