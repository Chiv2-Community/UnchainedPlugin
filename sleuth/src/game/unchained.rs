use std::os::raw::c_void;

use crate::{game::engine::TSoftClassPtr, ue::{FString, TArray, TMap, UClass, UObject}};

#[repr(C)]
pub struct DA_ModMarker_C {
    _private: [u8; 0x30], 
    pub mod_actors: TMap<*mut UClass, FString>,    // Offset 0x0030
    pub custom_objects: TMap<*mut UObject, FString>, // Offset 0x0080
}

#[repr(C)]
pub struct ArgonSDKModBase {
    pub actor_padding: [u8; 0x258],
    // 0x0258
    pub uber_graph_frame: u64, 
    // 0x0260
    pub default_scene_root: *mut c_void, 
    // 0x0268
    pub mod_name: FString,
    // 0x0278
    pub mod_version: FString,
    // 0x0288
    pub mod_description: FString,
    // 0x0298
    pub author: FString,
    // 0x02A8
    pub b_silent_load: bool,
    pub padding_0: [u8; 0x7], // 0x02A9
    // 0x02B0
    pub mod_repo_url: FString,
    // 0x02C0
    pub duplicate: bool,
    pub b_enable_by_default: bool,
    pub b_show_in_gui: bool,
    pub b_online_only: bool,
    pub b_host_only: bool,
    pub b_allow_on_frontend: bool,
    pub padding_1: [u8; 0x2], // 0x02C6
    // 0x02C8
    pub mod_version_repl: FString,
    // 0x02D8
    // pub ingame_mod_menu_widgets: TMap<FString, *mut c_void>,
    // 0x0328
    // pub b_clientside: bool,
}

#[repr(C)]
pub struct UModLoaderSettings_C {
    // 0x0000 (size 0x28) 
    pub base: [u8; 0x28], 
    // 0x0028 (size: 0x8)
    pub uber_graph_frame: u64, 
    // 0x0030 (size: 0x10)
    pub last_active_mods: TArray<FString>,
    // 0x0040 (size: 0x10)
    pub last_active_map: FString,
    // 0x0050 (size: 0x10)
    pub last_active_category: FString,
    // 0x0060 (size: 0x01)
    pub b_enable_bots: bool,
    // 0x0061 (size: 0x03) - Padding
    pub padding_0: [u8; 0x3],
    // 0x0064 (size: 0x04)
    pub num_player_bots: i32,
    // 0x0068 (size: 0x01)
    pub test: bool,
    // 0x0069 (size: 0x01)
    pub test_0: u8,
    // 0x006A (size: 0x02) - Padding
    pub padding_1: [u8; 0x2],
    // 0x006C (size: 0x04)
    pub test_1: i32,
    // 0x0070 (size: 0x04)
    pub test_2: f32,
    // 0x0074 (size: 0x04) - Padding
    pub padding_2: [u8; 0x4],
    // 0x0078 (size: 0x10)
    pub test_3: FString,
    // 0x0088 (size: 0x18)
    pub test_4_pad: [u8; 0x18], 
    // pub test_4: FText,
    // 0x00A0 (size: 0x10)
    pub test_5: TArray<FString>,
    // 0x00B0 (size: 0x50)
    pub test_6_pad: [u8; 0x50], 
    // pub test_6: TSet<FString>,
    // 0x0100 (size: 0x50)
    pub test_7_pad: [u8; 0x50], 
    // pub test_7: TMap<FString, i32>,
    // 0x0150 (size: 0x10)
    pub enabled_mods: TArray<TSoftClassPtr>,
    // 0x0160 (size: 0x01)
    pub b_use_player_bots: bool,
    // 0x0161 (size: 0x01)
    pub b_show_gui: bool,
}