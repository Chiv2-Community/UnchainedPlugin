use crate::tools::hook_globals::cli_args;

define_pattern_resolver!(
    TickActor,
    ["88 54 24 10 55 53 41 54 41 55 48 8D AC 24 E8 F7 FF FF 48 81 EC 18 09 00 00 45",]
);
CREATE_PATCH!(TickActor, 0x73, NOP, 6, IF { cli_args().tick_actor_patch });
