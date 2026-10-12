
// UCharacterMovementComponent::MoveAutonomous + 0x2f9
define_pattern_resolver!(MoveAutonomous, [
    "48 89 5C 24 08 57 48 81 EC D0 00 00 00 48 8B 01 41 0F B6 F9 0F 29 B4 24",
]);
// CREATE_PATCH!(MoveAutonomous, 0x2f9, BYTES, &[0xEB], IF { cli_args().tick_actor_patch });  // jmp
CREATE_PATCH!(MoveAutonomous, 0x2f6, BYTES, &[0xE9, 0xBE, 0x0, 0x0, 0x0, 0x90], IF { cli_args().tick_actor_patch });  // jmp to ret

define_pattern_resolver!(TickActor, [
    "88 54 24 10 55 53 41 54 41 55 48 8D AC 24 E8 F7 FF FF 48 81 EC 18 09 00 00 45",
]);
CREATE_PATCH!(TickActor, 0x73, NOP, 6, IF { cli_args().tick_actor_patch }); // spam NOP
