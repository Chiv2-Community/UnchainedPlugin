//! Legacy console-authentication bypass; exact native source owner is unresolved.

CREATE_PATCH!(AuthenticateConsoleCommandCaller, BYTES, &[0xEB]);
