

use std::{ffi::c_void, os::raw::{c_char, c_longlong}};


/*KismetExecutionMessage*/
	
use crate::{globals, sinfo, ue::*};
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


#[cfg(feature = "dev-joindata")]
use {
	futures::future::join_all,
	patternsleuth::{resolvers::ensure_one, scanner::Pattern},
};

// 48 8d 55 97 48 8d 4d d7 e8 ?? ?? ?? ??
// This is actually something else than I was looking for
// [20:09:50 INFO ] [JoinData_detour_fkt] Triggered! /Client/GetTitleData
// [20:09:50 INFO ] [JoinData_detour_fkt] Triggered! /Client/GetCatalogItems
// [20:09:50 INFO ] [JoinData_detour_fkt] Triggered! /Client/GetUserInventory
// [20:09:50 INFO ] [JoinData_detour_fkt] Triggered! /Authentication/GetEntityToken
// [20:09:50 INFO ] [JoinData_detour_fkt] Triggered! /Client/GetCatalogItems
// [20:09:50 INFO ] [JoinData_detour_fkt] Triggered! /Match/CancelAllMatchmakingTicketsForPlayer
// [20:09:50 INFO ] [JoinData_detour_fkt] Triggered! /MultiplayerServer/ListQosServersForTitle
#[cfg(feature = "dev-joindata")]
define_pattern_resolver!(JoinData, [
	"48 8d 55 97 48 8d 4d d7 e8 | ?? ?? ?? ??",
	], 
|ctx, patterns| {
	let res = futures::future::join_all(patterns.iter().map(|p| ctx.scan(patternsleuth::scanner::Pattern::new(p).unwrap()))).await;
	
	let temp_res = res.iter()
			.flatten()
			.map(|a| -> patternsleuth::resolvers::Result<usize> { Ok(patternsleuth::MemoryTrait::rip4(&ctx.image().memory, *a)?) });
	let len = temp_res.clone().count();
    let lvec: Vec<Result<usize, patternsleuth::resolvers::ResolveError>> = temp_res.clone().collect();
	println!("JoinData: {:#?}", lvec);
	// patternsleuth::resolvers::try_ensure_one(lvec.get(1).unwrap())?
	*lvec.get(3).unwrap().as_ref().unwrap() // 2 and 3 point to the correct one
});

#[cfg(feature = "dev-joindata")]
CREATE_HOOK!(JoinData, *mut u8, (arg0: *mut u8, arg1: *mut FString),{
	let string_ref: &FString = unsafe{ &*arg1 };
    crate::sinfo![f; "Triggered! {string_ref}"];
});
// // INVALID
// #[cfg(feature = "dev")]
// define_pattern_resolver!(JoinDataTwo,{
// 	OTHER: [
// 		"48 89 5C 24 08 48 89 6C 24 18 48 89 74 24 20 57 48 83 EC 70 49 8B F0 48 8B DA 48 8B E9 ?? ?? ?? ?? ?? 48 8B F8 48 8D 4C 24 30 33",
// 		// "48 89 5C 24 08 48 89 6C 24 18 48 89 74 24 20 57 48 83 EC 70 49 8B F0 48 8B DA 48 8B E9 ? ? ? ? ? 48 8B F8 48 8D 4C 24 30 33"
// 		]
	
// });
// #[cfg(feature = "dev")]
// CREATE_HOOK!(JoinDataTwo, c_void, (arg0: c_void, arg1: *mut c_void, arg3: c_void),{
//     crate::sinfo![f; "Triggered!"];
// });



#[cfg(feature = "dev")]
define_pattern_resolver!(JoinDataTwo,{
	OTHER: [
		// "24 48 48 8B 07 48 8B CF 48 8B 15",
		"4C 8B DC 48 83 EC 38 33 C0 49 89 5B 08 49 89 43 E8 49 89 43 F0 49 8D 43 E8 49 89 7B F8 48"
		//"4C 8B DC 48 83 EC 38 33 C0 49 89 5B 08 49 89 43 E8 49 89 43 F0 49 8D 43 E8 49 89 7B F8 48 8B F9 48 3B C2 ? ? 48 63 5A 08 49 89 73 10 48 8B 32 89 5C 24 28 85 DB ? ? 45 33 C0 49 8D 4B E8 8B D3 ? ? ? ? ? 48 8B 4C 24 20 4C 8B C3 48 8B D6 ? ? ? ? ? 48 8B 74 24 48 48 8B 07 48 8B CF 48 8B 15 90 77"
		// "48 89 5C 24 08 48 89 6C 24 18 48 89 74 24 20 57 48 83 EC 70 49 8B F0 48 8B DA 48 8B E9 ? ? ? ? ? 48 8B F8 48 8D 4C 24 30 33"
		]
	
}, |ctx, patterns| {
	let futures = ::patternsleuth::resolvers::futures::future::join_all(
		patterns.iter()
			.map(|p| ctx.scan(::patternsleuth::scanner::Pattern::new(p).unwrap()))
	).await;

    let lvec = futures.into_iter().flatten();
    let lvec2: Vec<usize> = lvec.clone().collect();
	// println!("JoinData: {:#?}", lvec);
	// patternsleuth::resolvers::try_ensure_one(lvec.get(1).unwrap())?

	// let mut fns = patternsleuth::resolvers::unreal::util::root_functions(ctx, &lvec2)?;
    // // crate::sinfo![f; "Funcs: {:#?}", fns];
	let mut cnt = 0;
	let base_addr = globals().get_base_address();
	// fns.clone().into_iter().for_each(|x| {
	// 	crate::sinfo![f; "{}: 0x{:#?}", cnt, x - base_addr];
	// 	cnt += 1;
	// });
	lvec2.clone().into_iter().for_each(|x| {
		crate::sinfo![f; "{}: 0x14{:X?}", cnt, x - base_addr];
		cnt += 1;
	});
	// *fns.get(0).unwrap() // 2 and 3 point to the correct one
	// 0x0cdabd0 + base_addr
	let val: usize = 0x1b5dba0;
	let offset:usize = (0x1b5dba0 & 0xFFFFFFF);
	let sel_addr: usize = offset + base_addr;

    // crate::sinfo![f; "Selected: 0x{sel_addr:X?}, base 0x{base_addr:X?}, offset 0x{offset:X?} TEST: 0x{val:X?}"];
	// [21:49:25 INFO ] [resolver] Selected: 0x140699541494688, base 0x140699512799232, offset 0x28695456 TEST: 0x28695456
	let sel2 = *lvec2.get(1).unwrap();
	let sel_temp: usize = sel2 - base_addr;
	crate::sinfo![f; "Selected: 0x{sel_temp:X?}"];
	// sel_addr
	// sel2
	0x1951330 + base_addr
	// ::patternsleuth::resolvers::ensure_one()?
});

#[cfg(feature = "dev")]
// CREATE_HOOK!(JoinDataTwo, c_void, (arg0: *mut c_void, str2: *mut c_void),{
// CREATE_HOOK!(JoinDataTwo, c_void, (param_1: *mut c_longlong, param_2: *mut *mut c_void),{
CREATE_HOOK!(JoinDataTwo, c_void, (param_1: u64, param_2: *mut u16, param_3: *mut c_char),{ // some other
	unsafe {

		// let my_string = std::ffi::CString::new("Hello world").expect("CString::new failed");
		// // let ptr: *mut c_char 
		// let raw = my_string.into_raw(); // takes ownership, must free later
	
		// *param_3 = *raw;
		// unsafe {
		// 	// Use `ptr` as needed...
		// 	println!("ptr = {}", std::ffi::CStr::from_ptr(param_3).to_string_lossy());
	
		// 	// When done, free the memory
		// 	let _ = std::ffi::CString::from_raw(param_3); // Reclaim to drop it
		// }


		// let fstring = FString::from(
		// 	widestring::U16CString::from_ptr_str(param_2)
		// 	.as_slice_with_nul());
		// // let string_ref: &FString = unsafe{ &*arg1 };
		// // let message = string_ref.to_string();
		// println!("JoinDataTwo: {}", fstring);
		
		log::info!("C string: {}", std::ffi::CStr::from_ptr(param_3).to_string_lossy());
		
		crate::sinfo![f; "Triggered!"];
		println!("ASDFG");
	}
});

// Disables startup message
// InitializeModule?
// FIXME: Nihi: offset in macro
// xref search + offset
// 11.06.25 Steam 19626AE + 0xF = 19626BD
//    19626[ae]		 4c 8d 05    LEA    R8,[s_Ple
//           		 3b 30 dd 02
//    19626b5		 48 8b cf    MOV    RCX,RDI
//    19626b8		 48 8d 54    LEA    RDX,[RSP + 0x50]
//           		 24 50
//    19626[bd]		 {e8 6e ec}    CALL   FUN_141951330  
//           		 {fe ff}
//    19626c2		 48 85 db    TEST   RBX,RBX
//    19626c5		 74 08       JZ     LAB_1419626cf
#[cfg(feature = "dev")]
define_pattern_resolver!(ShowSusMessage, [
    patternsleuth::resolvers::unreal::util::utf8_pattern("Please, start the game")
], |ctx, patterns| {
	let strings = ctx.scan(patterns.first().unwrap().clone()).await;
	let refs: Vec<usize> = patternsleuth::resolvers::unreal::util::scan_xrefs(ctx, &strings).await;
	patternsleuth::resolvers::ensure_one(refs)?	+ 0xF
});

#[cfg(feature = "dev")]
CREATE_HOOK!(ShowSusMessage, c_void, (param_1: c_longlong),{
	println!("ASDFG");
    crate::sinfo![f; "Triggered!"];
});

// #[cfg(feature = "dev")]
// define_pattern_resolver!(JoinDataFour, [
//     "DE AD BE EF AA AA AA AA AA"
// ], 
// |ctx, patterns| {
// 	// 0x1d25da0 + globals().get_base_address()
// });

// #[cfg(feature = "dev")]
// CREATE_HOOK!(JoinDataFour, c_void, (param_1: *mut FString),{
// 	let string_ref: &FString = unsafe{ &*param_1 };
// 	println!("ASDFG {:#?}", string_ref);
//     crate::sinfo![f; "Triggered!"];
// });

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
