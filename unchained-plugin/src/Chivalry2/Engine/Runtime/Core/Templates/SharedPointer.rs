//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/Templates/SharedPointer.h>.

use std::ffi::c_void;

#[unchained_macros_core::runtime_bindings]
pub type TSharedRefIHttpRequest = c_void; // TSharedRef<IHttpRequest, 0>

#[unchained_macros_core::runtime_bindings]
pub type TSharedPtrIHttpRequest = c_void; // TSharedPtr<IHttpRequest, 0>
