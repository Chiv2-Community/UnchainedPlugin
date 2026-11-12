use std::os::raw::c_void;

use crate::{
    events::broadcast_message::game_chat::broadcast_parts,
    game::{chivalry2::EChatType, engine::{FText, UWorld}},
    resolvers::admin_control::o_FText_AsCultureInvariant,
    ue::{FName, FString},
};

define_pattern_resolver!(ClientReceiveLocalizedChat, [
    "48 89 5C 24 18 55 57 41 56 48 81 EC 90 00 00 00 48 8B 99 D0 02 00 00 41 8B E8 4C 8B F2 48 8B F9"
]);
// ATBLPlayerController::ClientReceiveLocalizedChat_Implementation(const FText&, EChatType)
CREATE_HOOK!(ClientReceiveLocalizedChat, ACTIVE, NONE, (), (
    controller: *mut c_void, message: *const FText, channel: u32
), {
    if let Some(raw) = native_string(message) {
        if display_broadcast(controller, channel, &raw).is_some() {
            return;
        }
    }
    CALL_ORIGINAL!(ClientReceiveLocalizedChat(controller, message, channel));
});

define_pattern_resolver!(TBLGameInstance_AddChatLine, [
    "48 89 6C 24 18 56 41 56 41 57 48 83 EC 40 49 63 E9 4D 8B F8 4C 8B F2 48 8B F1"
]);
// UTBLGameInstance::AddChatLine: local display helper, not an intercepted function.
CREATE_HOOK!(TBLGameInstance_AddChatLine, CALLED, (), (
    instance: *mut c_void, sender: *const FText, message: *const FText,
    channel: u32, want_focus: bool, is_sender_dev: bool, origin_color: *const [f32; 4]
), {});

fn display_parts(channel: u32, message: &str) -> Option<(&str, &str, bool, [f32; 4])> {
    if channel != EChatType::AllSay as u32 && channel != EChatType::ServerSay as u32 {
        return None;
    }

    let (title, body, color) = broadcast_parts(message)?;
    // Presentation only: this hook receives localized system chat, not player commands.
    let is_sender_dev = title == "Server Console";
    let rgb = if is_sender_dev { 0x00FFFF } else { color.unwrap_or(0xFFFFFF) };
    let [r, g, b] = [16, 8, 0].map(|shift| {
        let srgb = ((rgb >> shift) & 0xFF) as f32 / 255.0;
        if srgb <= 0.04045 {
            srgb / 12.92
        } else {
            ((srgb + 0.055) / 1.055).powf(2.4)
        }
    });
    Some((title, body, is_sender_dev, [r, g, b, 1.0]))
}

fn display_broadcast(controller: *mut c_void, channel: u32, raw: &str) -> Option<()> {
    use super::client_message::{ClientMessage_detour_fkt, o_ClientMessage};
    use crate::resolvers::gameplay::object_inherits_from;

    let (title, body, is_sender_dev, origin_color) = display_parts(channel, raw)?;
    if controller.is_null() { return None; }
    // Match ClientReceiveLocalizedChat's Player guard and virtual GetWorld call.
    // Supported Steam/Epic ABI: APlayerController::Player +0x2d0, GetWorld slot +0x150.
    let player = unsafe { controller.byte_add(0x2d0).cast::<*mut crate::ue::UObject>().read() };
    if !object_inherits_from(player, "Player") { return None; }
    let get_world: unsafe extern "C" fn(*mut c_void) -> *mut UWorld = unsafe {
        let vtable = controller.cast::<*const usize>().read();
        std::mem::transmute(*vtable.add(0x150 / std::mem::size_of::<usize>()))
    };
    let world = unsafe { get_world(controller).as_ref() }?;
    let instance = world.owning_game_instance;
    if !object_inherits_from(instance.cast(), "TBLGameInstance") { return None; }
    let title = NativeText::new(title)?;
    let body = NativeText::new(body)?;
    // Preflight both calls before producing any output; failures use the untouched native path.
    if (!o_TBLGameInstance_AddChatLine.is_enabled() && unsafe { o_TBLGameInstance_AddChatLine.disable().is_err() })
        || (!o_ClientMessage.is_enabled() && unsafe { o_ClientMessage.disable().is_err() })
    {
        return None;
    }
    let mut console_line = FString::from(format!("<{channel}>: {raw}").as_str());
    // Use the existing capture hook as well as native console output, exactly once.
    unsafe { ClientMessage_detour_fkt(controller, &mut console_line, FName::default(), 0.0); }
    CALL_ORIGINAL!(TBLGameInstance_AddChatLine(instance, &title.0, &body.0, channel, false, is_sender_dev, &origin_color));
    Some(())
}

define_pattern_resolver!(FText_ToString, [
    "40 53 48 83 EC 20 48 8B D9 E8 ?? ?? ?? ?? 48 8B 0B 48 8B 01 48 83 C4 20 5B 48 FF 60 10"
]);
// FText::ToString returns a borrowed FString owned by the text data.
CREATE_HOOK!(FText_ToString, CALLED, *const FString, (text: *const FText), {});

fn native_string(text: *const FText) -> Option<String> {
    if text.is_null() { return None; }
    let string = CALL_ORIGINAL_SAFE!(FText_ToString(text)).ok()?;
    unsafe { string.as_ref() }?.copy_to_string().ok()
}

define_pattern_resolver!(FText_Release, [
    "40 53 48 83 EC 20 48 8B 59 08 48 85 DB 74 38 48 89 7C 24 30 BF FF FF FF FF 8B C7 F0 0F C1 43 08 83 F8 01 75 1D 48 8B 03 48 8B CB FF 10 F0 0F C1 7B 0C 83 FF 01 75 0B 48 8B 03 8B D7 48 8B CB FF 50 08 48 8B 7C 24 30 48 83 C4 20 5B C3"
]);
// Native thread-safe shared-reference destructor; FText's control block is at +8.
CREATE_HOOK!(FText_Release, CALLED, (), (text: *mut FText), {});

struct NativeText(FText);

impl NativeText {
    fn new(text: &str) -> Option<Self> {
        if !o_FText_Release.is_enabled() && unsafe { o_FText_Release.disable().is_err() } {
            return None;
        }
        let mut result = Self(FText::default());
        let mut string = FString::from(text);
        CALL_ORIGINAL_SAFE!(FText_AsCultureInvariant(&mut result.0, &mut string)).ok()?;
        Some(result)
    }
}

impl Drop for NativeText {
    fn drop(&mut self) {
        let _ = CALL_ORIGINAL_SAFE!(FText_Release(&mut self.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_text_layout_matches_windows_abi() {
        assert_eq!(std::mem::size_of::<FText>(), 24);
        assert_eq!(std::mem::align_of::<FText>(), 8);
        assert_eq!(std::mem::offset_of!(FText, flags), 16);
    }

    #[test]
    fn display_override_is_only_for_global_localized_broadcasts() {
        let message = "{$Map Change}\nStarting match on Wardenglade.";
        for channel in EChatType::ALL {
            let parts = display_parts(channel as u32, message);
            assert_eq!(parts.is_some(), matches!(channel, EChatType::AllSay | EChatType::ServerSay));
        }
        assert!(display_parts(u32::MAX, message).is_none());
        assert!(display_parts(EChatType::AllSay as u32, "Welcome\nEnjoy!").is_none());
        assert_eq!(display_parts(EChatType::AllSay as u32, message), Some(("Map Change", "Starting match on Wardenglade.", false, [1.0; 4])));
        assert!(display_broadcast(std::ptr::null_mut(), EChatType::AllSay as u32, message).is_none());
    }

    #[test]
    fn server_console_broadcasts_set_sender_dev_without_matching_player_chat() {
        let message = "{$Server Console}\nHello";
        for channel in [EChatType::AllSay, EChatType::ServerSay] {
            assert_eq!(display_parts(channel as u32, message),
                Some(("Server Console", "Hello", true, [0.0, 1.0, 1.0, 1.0])));
        }
        assert!(display_parts(EChatType::AllSay as u32, "Server Console: Hello").is_none());
        assert_eq!(display_parts(EChatType::AllSay as u32, "{$Server Console|#FF0000}\nHello"),
            Some(("Server Console", "Hello", true, [0.0, 1.0, 1.0, 1.0])));
    }

    #[test]
    fn broadcast_rgb_becomes_linear_origin_color_with_opaque_alpha() {
        let (_, body, is_sender_dev, color) = display_parts(
            EChatType::AllSay as u32, "{$Announcement|#FF8000}\nHello",
        ).unwrap();
        assert_eq!(body, "Hello");
        assert!(!is_sender_dev);
        assert_eq!([color[0], color[2], color[3]], [1.0, 0.0, 1.0]);
        assert!((color[1] - 0.2158605).abs() < 0.000001);
        let (_, _, _, black) = display_parts(EChatType::AllSay as u32, "{$Title|#000000}\nHello").unwrap();
        assert_eq!(black, [0.0, 0.0, 0.0, 1.0]);
    }
}
