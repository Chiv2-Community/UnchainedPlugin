//! Native source: <https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/Templates/Tuple.h>.

use std::fmt::Debug;

#[unchained_macros_core::runtime_bindings]
#[derive(Default, Debug)]
#[repr(C)]
pub(crate) struct TTuple<A, B> {
    pub(crate) a: A,
    pub(crate) b: B,
}
