//! Filename-comparison branch patch in FPakPlatformFile::Mount.

CREATE_PATCH!(AuthenticateConsoleCommandCaller, BYTES, &[0xEB]);
