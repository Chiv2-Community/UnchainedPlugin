//! Administrative console-command permission patches.

// Console-command permission bypass in UTBLLocalPlayer::Exec.
CREATE_PATCH!(UTBLLocalPlayer__Exec, BYTES, &[0xEB]);
CREATE_PATCH!(AuthenticateConsoleCommandCaller, BYTES, &[0xEB]);
