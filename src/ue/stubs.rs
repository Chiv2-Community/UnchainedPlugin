use std::ffi::c_void;

// Native types used only behind pointers; these aliases do not describe layouts
// or provide distinct type identities or thread-safety guarantees.
pub type FConfigCacheIni = c_void;
pub type UEngine = c_void;
pub type FWorldContext = c_void;
pub type UPendingNetGame = c_void;
pub type UTBLGameInstance = c_void;
pub type AGameModeBase = c_void;
