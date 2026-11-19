//! Existing instruction patch for the legacy TickActor resolver.

use crate::chivalry2::tbl::animation::tbl_character_anim_instance::TickActor;
use crate::tools::hook_globals::cli_args;

// Preserve existing bytes and activation. The supplied PDB identifies the target
// as StartAttack; the intended behavior of this legacy patch needs validation.
CREATE_PATCH!(TickActor, 0x73, NOP, 6, IF { cli_args().tick_actor_patch });
