//! Plugin branch patch in FTBLGameModule::StartupModule's PakFile lookup.

CREATE_PATCH!(AllowAlternateBackend, 0x4D, BYTES, &[0x90, 0xE9]);
