use std::os::raw::c_void;
use crate::{game::engine::{FActorSpawnParameters, FRotator}, ue::{FString, FVector, UClass}};

define_pattern_resolver!(GetGameInfo, {
    // "48 8B C4 48 89 58 ?? 48 89 50 ?? 55 56 57 41 54 41 55 41 56 41 57 48 8D A8 ?? ?? ?? ?? 48 81 EC E0 02 00 00", // Universal
    // From sigga
    STEAM: ["48 8B C4 48 89 58 ?? 48 89 50 ?? 55 56 57 41 54 41 55 41 56 41 57 48 8D A8 ?? ?? ?? ?? 48 81 EC b0 01 00 00 45 33 FF"], // STEAM
    OTHER: ["48 8B C4 48 89 58 ?? 48 89 50 ?? 55 56 57 41 54 41 55 41 56 41 57 48 8D A8 ?? ?? ?? ?? 48 81 EC E0 02 00 00 33 FF"], // PDB
});

// FIXME: When is this called?
CREATE_HOOK!(GetGameInfo, ACTIVE, POST,
    *mut FString, (ret_ptr: *mut FString, uWorld: *mut c_void),
    |ret_val: *mut FString| {
        unsafe {
            crate::sinfo!(f; "{}", (&*ret_val).to_string());
        }
        ret_val
    });

define_pattern_resolver!(ClientTravelInternal,["4C 8B DC 48 83 EC 58 33 C0 49 89 5B 08 49 89 6B 10 41 0F B6 E9 49 89 73 18 48"]);
// void __thiscall
// APlayerController::ClientTravelInternal
// 		  (APlayerController *this,FString *param_1,ETravelType param_2,bool param_3,FGuid *param_4)
CREATE_HOOK!(ClientTravelInternal, ACTIVE, c_void, (PC: *mut c_void, arg1: *mut FString, TravelType: u8, arg3: bool, guid: *mut c_void),{

	#[derive(Debug, serde::Deserialize)]
	#[allow(dead_code)]
	struct ConnectionInfo {
		Address: String,
		PartyId: Option<String>,
		PartySize: Option<u32>,
		PlayFabId: Option<String>,
		UnofficialTicket: Option<String>,
	}
	
	//127.0.0.1:7777&PartyId=1231231&PartySize=1&PlayFabId=123123&UnofficialTicket=12323
	fn parse_connection_info(raw: &str) -> Result<ConnectionInfo, Box<dyn std::error::Error>> {
        crate::sinfo!(f; "raw: {}", raw);
		let query_str = raw.replace('?', "&");
		let query_with_address = format!("Address={}", query_str);
		let info: ConnectionInfo = serde_urlencoded::from_str(&query_with_address)?;
		Ok(info)
	}
	
	let string_ref: &FString = unsafe{ &*arg1 };

	match parse_connection_info(string_ref.to_string().as_str()) {
        Ok(info) => {
			crate::sinfo!(f; "info: {:#?}", info);
        }
        Err(e) => eprintln!("Error: {}", e),
    }
});

// AActor * __thiscall UWorld::SpawnActor(UWorld *this,UClass *param_1,FVector *param_2,FRotator *param_3,FActorSpawnParameters *param_4)

define_pattern_resolver!(SpawnActor,["40 53 56 57 48 83 EC 70 48 8B 05 09 4B 63 02 48 33 C4 48 89 44 24 60 0F 28 1D E2 74"]);
CREATE_HOOK!(SpawnActor, INACTIVE, *mut c_void, (world: *mut c_void, class: *mut UClass, position: *mut FVector, rotation: *mut FRotator, spawn_params: *mut FActorSpawnParameters),{
    crate::sinfo![f; "Triggered!"];
});
/*
ATBLGameMode * __cdecl UTBLSystemLibrary::GetTBLGameMode(UObject *param_1)
*/
define_pattern_resolver!(GetTBLGameMode,["40 53 48 83 EC 20 48 8B D9 48 85 C9 ?? ?? 48 8B 01 ?? ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 0F 1F 40 00 48 8B 5B 20 48 85 DB ?? ?? 48 8B 03 48 8B CB ?? ?? ?? ?? ?? ?? 48 85 C0 ?? ?? 48 8B 98 28 01 00 00 48 85 DB ?? ?? ?? ?? ?? ?? ?? 48 8B 4B 10 48 83 C0 30 48 63 50 08 3B 51"]);
CREATE_HOOK!(GetTBLGameMode, INACTIVE, *mut c_void, (object: *mut c_void),{
    crate::sinfo![f; "Triggered!"];
});