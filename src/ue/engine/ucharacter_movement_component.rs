// UCharacterMovementComponent::MoveAutonomous + 0x2f9
use crate::tools::hook_globals::cli_args;
define_pattern_resolver!(
    MoveAutonomous,
    ["48 89 5C 24 08 57 48 81 EC D0 00 00 00 48 8B 01 41 0F B6 F9 0F 29 B4 24",]
);
// CREATE_PATCH!(MoveAutonomous, 0x2f9, BYTES, &[0xEB], IF { cli_args().tick_actor_patch });  // jmp
CREATE_PATCH!(MoveAutonomous, 0x2f6, BYTES, &[0xE9, 0xBE, 0x0, 0x0, 0x0, 0x90], IF { cli_args().tick_actor_patch }); // jmp to ret
