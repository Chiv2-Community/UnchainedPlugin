//! Plugin desync patch for UNetDriver::GetNetMode.

use crate::tools::hook_globals::cli_args;

CREATE_PATCH!(UNetDriver__GetNetMode, BYTES, &[0xB8, 0x01, 0x00, 0x00, 0x00, 0xC3], IF { cli_args().apply_desync_patch });
