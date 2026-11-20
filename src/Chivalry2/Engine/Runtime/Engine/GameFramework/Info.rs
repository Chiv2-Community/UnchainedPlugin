//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/GameFramework/Info.h>.

use crate::Chivalry2::Engine::*;

unsafe impl crate::Chivalry2::Engine::UObjectType for AInfo {
    const CLASS_NAME: &'static str = "Info";
}

#[unchained_macros::runtime_bindings]
#[rust_ctables::cpp_struct]
pub struct AInfo {
    #[inherit]
    pub base: AActor,
}
