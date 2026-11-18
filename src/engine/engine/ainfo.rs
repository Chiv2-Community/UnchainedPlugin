//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/GameFramework/Info.h>.

#![allow(non_snake_case)]
use crate::engine::*;

unsafe impl crate::ue::UObjectType for AInfo {
    const CLASS_NAME: &'static str = "Info";
}

#[rust_ctables::cpp_struct]
pub struct AInfo {
    #[inherit]
    pub base: AActor,
}
