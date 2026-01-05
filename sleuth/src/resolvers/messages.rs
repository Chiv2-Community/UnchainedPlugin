

// Chat messages
// #[cfg(feature="client_message")]
mod client_message {
    define_pattern_resolver!(ClientMessage, [
        "4C 8B DC 48 83 EC 58 33 C0 49 89 5B 08 49 89 73 18 49 8B D8 49 89 43 C8 48 8B F1 49 89 43 D0 49 89 43 D8 49 8D 43"
    ]);
    use std::os::raw::c_void;
    use crate::ue::{FName, FString};

    CREATE_HOOK!(ClientMessage, (this:*mut c_void, S:*mut FString, Type:FName, MsgLifeTime: f32), {
        let string_ref: &FString = unsafe{ &*S };
        let message = string_ref.to_string();
        
        crate::sinfo!("ClientMessage Hooked: Type: {:?}, Message: {}", Type, message);
    });
}

// Kismet error messages
#[cfg(feature="kismet_log")]
pub mod kismet_log {
    use crate::ue::{FName, UObject};
    
    // Chiv is spamming these from time to time. Shame, Shame, Shame
    // TODO: Maybe make it dynamic
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
    CREATE_HOOK!(KismetExecutionMessage, *mut UObject, (Message:*const u16, Type: u8, fname: FName), {
        
        if !Message.is_null() {
            unsafe {
                let msg = widestring::U16CStr::from_ptr_str(Message);
                let mut message = msg.to_string_lossy();
                message = match message.contains('\n') {
                    true => format!("\n{message}"),
                    false => message,
                };
                
                match LIST_OF_SHAME.iter().any(|x| message.contains(x)) {
                    true => {}
                    false => log::debug!(target: "kismet", "{message}"),
                }
            }
        }

    });
}
