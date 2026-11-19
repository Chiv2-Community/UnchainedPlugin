//! UE 4.25-plus: <https://github.com/EpicGames/UnrealEngine/tree/4.25-plus/Engine/Source/Runtime/Core/Public/Serialization>.

pub mod archive;
pub mod large_memory_reader;

pub use self::archive::*;
pub use self::large_memory_reader::*;
