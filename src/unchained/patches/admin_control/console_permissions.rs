//! Console-command permission bypass in UTBLLocalPlayer::Exec.

CREATE_PATCH!(UTBLLocalPlayer_Exec, BYTES, &[0xEB]);
