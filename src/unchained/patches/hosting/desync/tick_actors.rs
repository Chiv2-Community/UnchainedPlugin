//! Tick-actor desync patches.

use crate::tools::hook_globals::cli_args;
use crate::Chivalry2::TBL::TBLCharacterAnimInstance::TickActor;
use crate::Chivalry2::Engine::Runtime::Engine::GameFramework::CharacterMovementComponent::UCharacterMovementComponent__MoveAutonomous;
use unchained_macros_core::{register_patch, Patch};

// Preserve existing bytes and activation. An older build identifies the target
// as StartAttack; the intended behavior of this legacy patch needs validation.
#[register_patch(TickActor)]
fn skip_tick_actor(_resolved_address: u64) -> Patch {
    Patch::nop(0x73, 6).when(cli_args().tick_actor_patch)
}

// The earlier 0x2f9/EB variant is superseded by this jump to the return.
#[register_patch(UCharacterMovementComponent__MoveAutonomous)]
fn skip_autonomous_movement(_resolved_address: u64) -> Patch {
    Patch::bytes(0x2f6, &[0xE9, 0xBE, 0x0, 0x0, 0x0, 0x90]).when(cli_args().tick_actor_patch)
}
