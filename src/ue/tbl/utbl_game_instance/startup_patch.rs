//! Disable the startup anti-cheat message.
define_pattern_resolver!(
    EACAntiCheatMesssage,
    Simple,
    ["4c 8d 05 ?? ?? ?? ?? 48 8b cf 48 8d ?? ?? ?? ?? ?? fe ff 48 85 db 74 08"]
);
CREATE_PATCH!(EACAntiCheatMesssage, 0xE, NOP, 5);
