//! Tick-actor desync patches.

use crate::tools::hook_globals::cli_args;

// Preserve existing bytes and activation. The supplied PDB identifies the target
// as StartAttack; the intended behavior of this legacy patch needs validation.
CREATE_PATCH!(TickActor, 0x73, NOP, 6, IF { cli_args().tick_actor_patch });

// CREATE_PATCH!(MoveAutonomous, 0x2f9, BYTES, &[0xEB], IF { cli_args().tick_actor_patch });  // jmp
CREATE_PATCH!(MoveAutonomous, 0x2f6, BYTES, &[0xE9, 0xBE, 0x0, 0x0, 0x0, 0x90], IF { cli_args().tick_actor_patch }); // jmp to ret
