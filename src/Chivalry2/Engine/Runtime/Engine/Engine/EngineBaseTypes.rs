//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Engine/Classes/Engine/EngineBaseTypes.h>.

#![allow(non_camel_case_types)]

#[unchained_macros::runtime_bindings]
#[repr(C)]
pub enum ENetMode {
    STANDALONE,
    DEDICATED_SERVER,
    LISTEN_SERVER,
    CLIENT,
    MAX,
}
