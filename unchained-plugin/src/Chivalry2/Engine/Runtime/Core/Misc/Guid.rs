//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/Misc/Guid.h>.

use std::ffi::c_void;

// Native types used only behind pointers; these aliases do not describe layouts
// or provide distinct type identities or thread-safety guarantees.
#[unchained_macros_core::runtime_bindings]
pub type FGuid = c_void;
