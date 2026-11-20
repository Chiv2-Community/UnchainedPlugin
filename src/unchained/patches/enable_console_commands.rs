//! Administrative console-command permission patches.

// Console-command permission bypass in UTBLLocalPlayer::Exec.
CREATE_PATCH!(UTBLLocalPlayer_Exec, BYTES, &[0xEB]);
CREATE_PATCH!(AuthenticateConsoleCommandCaller, BYTES, &[0xEB]);
