use std::os::raw::c_void;

use crate::{backend_url, tools::hook_globals::globals, ue::FString};




define_pattern_resolver!(FString_AppendChars, [
    "45 85 C0 0F 84 89 00 00 00 48 89 5C 24 18 48 89 6C 24 20 56 48 83 EC 20 48 89 7C 24 30 48 8B EA 48 63 79 08 48 8B D9 4C 89 74 24 38 45 33 F6 85 FF 49 63 F0 41 8B C6 0F 94 C0 03 C7 03 C6 89 41 08 3B 41 0C 7E 07 8B D7 E8 ?? ?? ?? ?? 85 FF 49 8B C6 48 8B CF 48 8B D5 0F 95 C0 48 2B C8 48 8B 03 48 8D 1C 36 4C 8B C3 48 8D 3C 48 48 8B CF E8 ?? ?? ?? ?? 48 8B 6C 24 48 66 44 89 34 3B 4C 8B 74 24 38 48 8B 7C 24 30 48 8B 5C 24 40 48 83 C4 20 5E C3" // Universal
]);
CREATE_HOOK!(FString_AppendChars, ACTIVE, NONE, (), 
    (this_ptr: *mut FString, str_ptr: *const u16, count: u32), 
{
    CALL_ORIGINAL!(FString_AppendChars(this_ptr, str_ptr, count));
});

define_pattern_resolver!(
    PreLogin,
    XrefLast,
    [patternsleuth::resolvers::unreal::util::utf8_pattern(
        " Minutes"
    )]
);
fn is_user_banned(addr_wide: &[u16]) -> bool {
    let mut suffix = "/api/v1/check-banned/".to_string();
    suffix.push_str(&String::from_utf16_lossy(addr_wide));
    let url = backend_url!(suffix);
    let response = ureq::get(&url.to_string()).call();

    match response {
        Ok(res) => {
            match res.into_body().read_to_string() {
                Ok(body_str) => body_str.contains("true"),
                _ => false
            }
        }
        Err(e) => {
            crate::sinfo!(f; "Ban check failed: {:?}", e);
            // Do we return true here?
            false
        }
    }
}
CREATE_HOOK!(PreLogin, ACTIVE, NONE, (), (
    this_ptr: *mut c_void, // ATBLGameMode
    _options: *const FString, 
    address: *const FString, 
    unique_id: *const c_void, // FUniqueNetIdRepl
    error_message: *mut FString
), {
    if !globals().cli_args.use_backend_banlist {
        return unsafe { o_PreLogin.call(this_ptr, _options, address, unique_id, error_message) };
    }

    unsafe { o_PreLogin.call(this_ptr, _options, address, unique_id, error_message) };

    unsafe {
        // Join already failed for a different reason
        if !(*error_message).is_empty() {
            return;
        }
    }

    let addr_raw = unsafe { (*address).as_slice() }; 
    
    if !is_user_banned(addr_raw) {
        let msg = "You are banned from this server.";
        let wide_msg: Vec<u16> = msg.encode_utf16().collect();
        
        unsafe {
            o_FString_AppendChars.call(
                error_message, 
                wide_msg.as_ptr(), 
                wide_msg.len() as u32
            );
        }
        #[cfg(feature="verbose_hooks")]
        crate::swarn!(f; "User banned!");
    }
    else {
        #[cfg(feature="verbose_hooks")]
        crate::sinfo!(f; "User is not banned!");
    }
});


define_pattern_resolver!(ApproveLogin, [
    "48 89 5C 24 18 48 89 74 24 20 55 57 41 54 41 55 41 56 48 8D 6C 24 C9 48 81 EC A0 00 00 00 8B", // EGS
    "48 89 5C 24 10 48 89 74 24 18 55 57 41 54 41 56 41 57 48 8B EC 48 81 EC 80 00 00 00 8B", // STEAM
]);

define_pattern_resolver!(SendRequest, [
    "48 89 5C 24 ?? 48 89 74 24 ?? 48 89 7C 24 ?? 55 41 54 41 55 41 56 41 57 48 8B EC 48 83 EC 40 48 8B D9 49 8B F9"
]);

