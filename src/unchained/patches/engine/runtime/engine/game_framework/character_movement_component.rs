//! Plugin movement patch for UCharacterMovementComponent::MoveAutonomous.

use crate::tools::hook_globals::cli_args;

// CREATE_PATCH!(MoveAutonomous, 0x2f9, BYTES, &[0xEB], IF { cli_args().tick_actor_patch });  // jmp
CREATE_PATCH!(MoveAutonomous, 0x2f6, BYTES, &[0xE9, 0xBE, 0x0, 0x0, 0x0, 0x90], IF { cli_args().tick_actor_patch }); // jmp to ret
