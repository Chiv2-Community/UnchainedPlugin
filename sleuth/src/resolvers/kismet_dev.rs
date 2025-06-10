

use std::ffi::c_void;

/*KismetExecutionMessage*/
	
use crate::{sinfo, ue::*};
static LIST_OF_SHAME: [&str; 4] = [
	"/Game/Maps/Frontend/CIT/FE_Citadel_Atmospherics.FE_Citadel_Atmospherics_C",
	"Divide by zero: ProjectVectorOnToVector with zero Target vector",
	"A null object was passed as a world context object to UEngine::GetWorldFromContextObject().",
	"/Game/Maps/Frontend/Blueprints/Customization_Rotation.Customization_Rotation_C",
];

define_pattern_resolver!(KismetExecutionMessage, [
    "48 89 5C 24 08 57 48 83 EC 30 0F B6 DA 48 8B F9 80 FA 01 ?? ?? ?? ?? ?? ?? ?? ?? ?? BA",
]);

// void __cdecl FFrame::KismetExecutionMessage(wchar_t *param_1,Type param_2,FName param_3)

#[cfg(feature="kismet-log")]
CREATE_HOOK!(KismetExecutionMessage, *mut UObject, (Message:*const u16, Type: u8, fname: FName), {
	
    if !Message.is_null() {
        unsafe {
			let msg = widestring::U16CStr::from_ptr_str(Message);
			// let string =  FString::from(msg.as_slice_with_nul());
			let mut message = msg.to_string_lossy();
			message = match message.contains('\n') {
				true => format!("\n{message}"),//.replace("\r\n", " "),
				false => message,
			};
			
			match LIST_OF_SHAME.iter().any(|x| message.contains(x)) {
				true => {} // filtered out
				false => log::debug!(target: "kismet", "{message}"),
			}
        }
    }

});

#[cfg(feature = "rpc-debug")]
define_pattern_resolver!(LogReliableRPCFailed,[
    "48 89 5C 24 08 57 48 83 EC 40 41 83 78 08 00 49 8B F8 48 8B D9 ?? ?? 49 8B 00 ?? ?? 48"
    ]);
    
// FIXME: Nihi: prints are messed up
// void __thiscall
// UNetConnection::LogReliableRPCFailed
// (UNetConnection* this, FInBunch* param_1, FString* param_2, int param_3)
#[cfg(feature = "rpc-debug")]
CREATE_HOOK!(LogReliableRPCFailed, c_void, (this_ptr: *mut c_void, arg1: *mut FString, arg2: u32), {
	if !arg1.is_null() {
		let string_ref: &FString = unsafe{ &*arg1 };
		let message = string_ref.to_string();
		if string_ref.len() < 1024 {
			println!("LogReliableRPCFailed: {}", message);
		}
		else {
			println!("LogReliableRPCFailed");
		}
	}
	else {
		println!("LogReliableRPCFailed");
	}
    // println!("LogReliableRPCFailed");
    // crate::sinfo![f; "Triggered!"];
});

#[cfg(feature = "rpc-debug")]
define_pattern_resolver!(LogReliableRPC,[
    "48 89 5C 24 10 48 89 6C 24 18 48 89 74 24 20 41 56 48 83 EC 20 48 8B 01 41 8B E8 48 8B DA 4C 8B F1 ?? ?? ?? ?? ?? ?? 48 8B C8 ?? ?? ?? ?? ?? 48 8B F0 48 85 C0 ?? ?? ?? ?? ?? ?? ?? 48 8B 4E 10 48"
    ]);

//void __thiscall ATBLCharacter::LogReliableRPC(ATBLCharacter *this,FName param_1,int param_2)
#[cfg(feature = "rpc-debug")]
CREATE_HOOK!(LogReliableRPC, c_void, (this_ptr: *mut c_void, arg1: FName, arg2: u32), {
	println!("LogReliableRPC: {}", arg1);
});


#[cfg(feature = "dev")]
define_pattern_resolver!(ClientTravelToSession,["48 89 5C 24 08 48 89 6C 24 10 48 89 74 24 18 57 48 83 EC 50 48 8B 79 30 33 ED 49 8B D8 8B F2"]);
#[cfg(feature = "dev")]
// Nihi: looks like this is never called
// bool __thiscall UGameInstance::ClientTravelToSession(UGameInstance *this,int param_1,FName param_2)
CREATE_HOOK!(ClientTravelToSession, bool, (this_ptr: *mut c_void, param: i32, Name: *mut FName), {
    sinfo![F; "{}", unsafe { &*Name }];
});

#[cfg(feature = "dev")]
define_pattern_resolver!(ClientTravelInternal,["4C 8B DC 48 83 EC 58 33 C0 49 89 5B 08 49 89 6B 10 41 0F B6 E9 49 89 73 18 48"]);
#[cfg(feature = "dev")]
// void __thiscall
// APlayerController::ClientTravelInternal
// 		  (APlayerController *this,FString *param_1,ETravelType param_2,bool param_3,FGuid *param_4)
CREATE_HOOK!(ClientTravelInternal, c_void, (PC: *mut c_void, arg1: *mut FString, TravelType: u8, arg3: bool, guid: *mut c_void),{

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
		let query_str = raw.replace('?', "&");
		let query_with_address = format!("Address={}", query_str);
		let info: ConnectionInfo = serde_urlencoded::from_str(&query_with_address)?;
		Ok(info)
	}
	
	let string_ref: &FString = unsafe{ &*arg1 };

	match parse_connection_info(string_ref.to_string().as_str()) {
        Ok(info) => {
			sinfo!(f; "info: {:#?}", info);
        }
        Err(e) => eprintln!("Error: {}", e),
    }
});