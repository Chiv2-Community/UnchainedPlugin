//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/Templates/SharedPointer.h>.

use std::ffi::c_void;

pub type TSharedRefIHttpRequest = c_void; // TSharedRef<IHttpRequest, 0>

pub type TSharedPtrIHttpRequest = c_void; // TSharedPtr<IHttpRequest, 0>
