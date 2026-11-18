//! Native source: <https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/TBLPlayerController.h>.

#![allow(non_snake_case)]

unsafe impl crate::ue::UObjectType for ATBLPlayerController {
    const CLASS_NAME: &'static str = "TBLPlayerController";
}

#[rust_ctables::cpp_struct]
#[derive(Debug)]
pub struct ATBLPlayerController {
    #[inherit]
    pub base: crate::ue::APlayerController,
    _private: [u8; 0x1070],
    pub bOnlineInventoryInitialized: bool,
    pub bPlayerCustomizationReceived: bool,
}
