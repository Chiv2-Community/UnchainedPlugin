//! Native source: <https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/OwnershipResponse.h>.

#[unchained_macros::runtime_bindings]
#[repr(C)]
#[derive(Debug)]
#[allow(non_snake_case)]
pub struct FOwnershipResponse {
    pub(crate) owned: bool,
    crowns: i32,
    gold: i32,
    usdCents: i32,
    levelType: u8,
    pub(crate) level: i32,
}
