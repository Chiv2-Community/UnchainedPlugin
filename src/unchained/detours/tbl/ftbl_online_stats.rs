//! Plugin replacements for `crate::chivalry2::tbl::ftbl_online_stats`.

use crate::chivalry2::tbl::ftbl_online_stats::*;

#[unchained_macros::bind_func_detour(FTBLOnlineStats::set_stat_inner)]
pub(crate) unsafe fn set_stat_inner_detour(_this: *mut FTBLOnlineStats, _p1: i32, _p2: i32) {}
