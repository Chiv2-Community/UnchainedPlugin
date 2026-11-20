//! Startup anti-cheat-message patch in UTBLGameInstance::Init.

CREATE_PATCH!(EACAntiCheatMesssage, 0xE, NOP, 5);
