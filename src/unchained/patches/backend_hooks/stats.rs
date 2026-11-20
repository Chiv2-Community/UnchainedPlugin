//! Backend statistics overrides.

use crate::Chivalry2::TBL::TBLOnlineStats::*;

#[unchained_macros::bind_func_detour(FTBLOnlineStats::SetStatInner)]
pub(crate) unsafe fn set_stat_inner_detour(_this: *mut FTBLOnlineStats, _p1: i32, _p2: i32) {
    // Do nothing. Don't report player stats on Unchained
}
