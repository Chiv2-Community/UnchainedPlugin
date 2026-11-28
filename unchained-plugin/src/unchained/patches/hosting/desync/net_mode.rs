//! Plugin desync patch for UNetDriver::GetNetMode.

use crate::tools::hook_globals::cli_args;
use crate::Chivalry2::Engine::Runtime::Engine::Engine::NetDriver::UNetDriver__GetNetMode;
use unchained_macros_core::{register_patch, Patch};

#[register_patch(UNetDriver__GetNetMode)]
fn force_server_net_mode(_resolved_address: u64) -> Patch {
    Patch::bytes(0, &[0xB8, 0x01, 0x00, 0x00, 0x00, 0xC3]).when(cli_args().apply_desync_patch)
}
