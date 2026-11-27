//! Administrative console-command permission patches.

use crate::Chivalry2::TBL::TBLLocalPlayer::UTBLLocalPlayer__Exec;
use crate::Chivalry2::Engine::Runtime::Engine::GameFramework::PlayerController::AuthenticateConsoleCommandCaller;
use unchained_macros_core::{register_patch, Patch};

// Console-command permission bypass in UTBLLocalPlayer::Exec.
#[register_patch(UTBLLocalPlayer__Exec)]
static ALLOW_LOCAL_CONSOLE_COMMANDS: Patch = Patch::bytes(0, &[0xEB]);

#[register_patch(AuthenticateConsoleCommandCaller)]
static ALLOW_AUTHENTICATED_CONSOLE_COMMANDS: Patch = Patch::bytes(0, &[0xEB]);
