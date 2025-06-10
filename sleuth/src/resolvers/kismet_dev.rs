
	
use crate::swarn;
/*KismetExecutionMessage*/
	
use crate::ue::*;
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
			let msg = widestring::U16CStr::from_ptr_str(Message as *const u16);
			match FString::try_from(msg.as_slice_with_nul()) {
				Ok(str) => {
					let mut message = msg.to_string_lossy();
					message = match message.contains('\n') {
						true => format!("\n{message}"),//.replace("\r\n", " "),
						false => message,
					};
					
					match LIST_OF_SHAME.iter().any(|x| message.contains(x)) {
						true => {} // filtered out
						false => log::debug!(target: "kismet", "{message}"),
					}
				},
				Err(e) => swarn!(f; "{e}")
			}
        }
    }

});
