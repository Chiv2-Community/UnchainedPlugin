use crate::ue::FString;


#[repr(C)]
#[derive(Debug)]
pub struct FOwnershipResponse {
	owned: bool,
	crowns: i32,
	gold: i32,
	usdCents: i32,
	levelType: u8,
	level: i32
}

define_pattern_resolver!(ATBLPlayerController__GetOwnershipFromPlayerControllerAndState, {
    EGS: ["40 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 B0 FD"], // EGS
    STEAM: [
        "40 55 56 41 54 41 55 41 56 41 57 48 8D AC 24 B8",
        "40 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 B0 FD" // EGS 2.11.4
        ], // STEAM
    OTHER: ["40 55 53 56 57 41 54 41 55 41 56 41 57 48 8d ac 24 38 fd"]// PDB
});


CREATE_HOOK!(ATBLPlayerController__GetOwnershipFromPlayerControllerAndState, ACTIVE, POST,
    *mut FOwnershipResponse, (result: *mut FOwnershipResponse, PlayerController: *mut c_void, PlayerState: *mut c_void, AssetIdToCheck: *mut c_void, BaseOnly: bool),
    | response: *mut FOwnershipResponse | {
        let resp = unsafe { response.as_mut().expect("Response was null") };
        
        resp.level = 0;
        resp.owned = true;
        resp
});

define_pattern_resolver!(ATBLPlayerController__CanUseLoadoutItem, {
    EGS: ["48 89 5C 24 08 48 89 74 24 10 55 57 41 55 41 56 41 57 48 8B EC 48 81 EC 80 00 00"], // EGS
    // "48 89 5C 24 08 48 89 74 24 18 55 57 41 55 41 56 41 57 48 8B EC 48 83 EC", // STEAM
    // from sigga
    // "48 89 5C 24 08 48 89 74 24 10 55 57 41 55 41 56 41 57 48 8B EC 48 81 EC 80 00 00", // EGS
    STEAM: [
        "48 89 5C 24 08 48 89 74 24 18 55 57 41 55 41 56 41 57 48 8B EC 48 83 EC 60 49 8B 31 33 FF C6 02 00",
        "48 89 5C 24 08 48 89 74 24 10 55 57 41 55 41 56 41 57 48 8B EC 48 81 EC 80 00 00" // EGS 2.11.4
        ], // STEAM
});


CREATE_HOOK!(ATBLPlayerController__CanUseLoadoutItem, ACTIVE, POST,
    *mut FOwnershipResponse, (this: *mut ATBLPlayerController, result: *mut FOwnershipResponse, InLoadOutSelection: *mut c_void, InItem: *mut c_void), 
    | response: *mut FOwnershipResponse | {
        let resp = unsafe { response.as_mut().expect("Response was null") };
        let res = unsafe { result.as_mut().expect("Response was null") };
        
        resp.level = 0;
        resp.owned = true;
        res.owned = true;
        resp
});

define_pattern_resolver!(
    ATBLPlayerController__CanUseCharacter,
    [
        "48 89 5C 24 08 48 89 6C 24 10 48 89 74 24 18 48 89 7C 24 20 41 56 48 83 EC 50 49 8B 18", // universal
    ]
);

CREATE_HOOK!(ATBLPlayerController__CanUseCharacter, ACTIVE, POST,
    *mut FOwnershipResponse, (this: *mut ATBLPlayerController, result: *mut FOwnershipResponse, CharacterSubclass: *mut c_void), 
    | response: *mut FOwnershipResponse | {
        let resp = unsafe { response.as_mut().expect("Response was null") };
        
        resp.level = 0;
        resp.owned = true;
        resp
});


define_pattern_resolver!(ATBLPlayerController__ConditionalInitializeCustomizationOnServer, {
    EGS: ["48 89 54 24 10 53 56 57 41 54 48 83 EC 78 48 8B 99 60 02 00 00 48 8B F2 0F B6"], // EGS
    STEAM: ["48 89 54 24 10 53 ?? 57 41 54 48 83 EC 78"], // STEAM
    // From Sigga
    // Did the function change?
    OTHER: ["41 54 48 81 EC 80 00 00 00 80 B9 F8 00 00 00 03 4C 8B E1 ?? ?? ?? ?? ?? ?? 80 B9 20 13 00 00 00 ?? ?? ?? ?? ?? ?? 80 B9 21"], // PDB
});

#[repr(C)]
#[derive(Debug)]
pub struct ATBLPlayerController { 
    _private: [u8; 0x1348],
	bOnlineInventoryInitialized: bool,
	bPlayerCustomizationReceived: bool,
}

use std::os::raw::c_void;
CREATE_HOOK!(ATBLPlayerController__ConditionalInitializeCustomizationOnServer, ACTIVE, PRE,
    c_void, (this: *mut ATBLPlayerController, player_state: *mut c_void), {
        let pc = unsafe { this.as_mut().expect("PlayerController was null") };
        
        pc.bOnlineInventoryInitialized = true;
        pc.bPlayerCustomizationReceived = true;
});