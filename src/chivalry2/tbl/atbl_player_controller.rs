//! ArgonSDK main: [ATBLPlayerController](https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/TBLPlayerController.h).

use std::os::raw::c_void;
use crate::chivalry2::engine::{
    FPrimaryAssetId, TSubclassOfAInventoryItem, TSubclassOfATBLCharacter,
    TSubclassOfULoadoutSelection, FText, FLinearColor, FName, FString, InvariantText,
};
use crate::chivalry2::tbl::{ATBLPlayerState, EChatType, UTBLGameInstance};
use crate::chivalry2::tbl::fownership_response::FOwnershipResponse;
use crate::events::broadcast_message::game_chat::broadcast_parts;

unsafe impl crate::chivalry2::engine::UObjectType for ATBLPlayerController {
    const CLASS_NAME: &'static str = "TBLPlayerController";
}

#[rust_ctables::cpp_struct]
#[derive(Debug)]
pub struct ATBLPlayerController {
    #[inherit]
    pub base: crate::chivalry2::engine::APlayerController,
    _private: [u8; 0x1070],
    pub bOnlineInventoryInitialized: bool,
    pub bPlayerCustomizationReceived: bool,
}

// Methods to enable customization / loadout on custom servers

define_pattern_resolver!(ATBLPlayerController__GetOwnershipFromPlayerControllerAndState, {
    EGS: ["40 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 B0 FD"], // EGS
    STEAM: [
        "40 55 56 41 54 41 55 41 56 41 57 48 8D AC 24 B8",
        "40 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 B0 FD" // EGS 2.11.4
        ], // STEAM
    OTHER: ["40 55 53 56 57 41 54 41 55 41 56 41 57 48 8d ac 24 38 fd"]// PDB
});

#[unchained_macros::runtime_bindings]
impl ATBLPlayerController {
    #[bind_resolved_func_detour(ATBLPlayerController__GetOwnershipFromPlayerControllerAndState)]
    pub(crate) unsafe fn get_ownership_from_player_controller_and_state_detour(
        result: *mut FOwnershipResponse,
        PlayerController: *mut ATBLPlayerController,
        PlayerState: *mut ATBLPlayerState,
        AssetIdToCheck: *mut FPrimaryAssetId,
        BaseOnly: bool,
    ) -> *mut FOwnershipResponse {
        // Caller-provided result storage precedes the controller in the native ABI.
        let response = unsafe {
            Self::get_ownership_from_player_controller_and_state(
                result,
                PlayerController,
                PlayerState,
                AssetIdToCheck,
                BaseOnly,
            )
        };
        if let Some(resp) = unsafe { response.as_mut() } {
            resp.level = 0;
            resp.owned = true;
        }
        response
    }
}

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

#[unchained_macros::runtime_bindings]
impl ATBLPlayerController {
    #[bind_resolved_func_detour(ATBLPlayerController__CanUseLoadoutItem)]
    pub(crate) unsafe fn can_use_loadout_item_detour(
        self: *mut Self,
        result: *mut FOwnershipResponse,
        InLoadOutSelection: *mut TSubclassOfULoadoutSelection,
        InItem: *mut TSubclassOfAInventoryItem,
    ) -> *mut FOwnershipResponse {
        let response = unsafe { self.can_use_loadout_item(result, InLoadOutSelection, InItem) };
        if let Some(resp) = unsafe { response.as_mut() } {
            resp.level = 0;
            resp.owned = true;
        }
        if let Some(res) = unsafe { result.as_mut() } {
            res.owned = true;
        }
        response
    }
}

define_pattern_resolver!(
    ATBLPlayerController__CanUseCharacter,
    [
        "48 89 5C 24 08 48 89 6C 24 10 48 89 74 24 18 48 89 7C 24 20 41 56 48 83 EC 50 49 8B 18", // universal
    ]
);

#[unchained_macros::runtime_bindings]
impl ATBLPlayerController {
    #[bind_resolved_func_detour(ATBLPlayerController__CanUseCharacter)]
    pub(crate) unsafe fn can_use_character_detour(
        self: *mut Self,
        result: *mut FOwnershipResponse,
        CharacterSubclass: *mut TSubclassOfATBLCharacter,
    ) -> *mut FOwnershipResponse {
        let response = unsafe { self.can_use_character(result, CharacterSubclass) };
        if let Some(resp) = unsafe { response.as_mut() } {
            resp.level = 0;
            resp.owned = true;
        }
        response
    }
}

define_pattern_resolver!(ATBLPlayerController__ConditionalInitializeCustomizationOnServer, {
    EGS: ["48 89 54 24 10 53 56 57 41 54 48 83 EC 78 48 8B 99 60 02 00 00 48 8B F2 0F B6"], // EGS
    STEAM: ["48 89 54 24 10 53 ?? 57 41 54 48 83 EC 78"], // STEAM
    // From Sigga
    // Did the function change?
    OTHER: ["41 54 48 81 EC 80 00 00 00 80 B9 F8 00 00 00 03 4C 8B E1 ?? ?? ?? ?? ?? ?? 80 B9 20 13 00 00 00 ?? ?? ?? ?? ?? ?? 80 B9 21"], // PDB
});

// The extra legacy player_state argument is not established by the native signature.
#[unchained_macros::runtime_bindings]
impl ATBLPlayerController {
    #[bind_resolved_func_detour(ATBLPlayerController__ConditionalInitializeCustomizationOnServer)]
    pub(crate) unsafe fn conditional_initialize_customization_on_server_detour(
        self: *mut Self,
        player_state: *mut c_void,
    ) -> c_void {
        let this = self;
        if let Some(pc) = unsafe { this.as_mut() } {
            pc.bOnlineInventoryInitialized = true;
            pc.bPlayerCustomizationReceived = true;
        }
        unsafe { self.conditional_initialize_customization_on_server(player_state) }
    }
}

define_pattern_resolver!(ClientReceiveLocalizedChat, [
    "48 89 5C 24 18 55 57 41 56 48 81 EC 90 00 00 00 48 8B 99 D0 02 00 00 41 8B E8 4C 8B F2 48 8B F9"
]);

// ATBLPlayerController::ClientReceiveLocalizedChat_Implementation(const FText&, EChatType)
#[unchained_macros::runtime_bindings]
impl ATBLPlayerController {
    #[bind_resolved_func_detour(ClientReceiveLocalizedChat)]
    unsafe fn client_receive_localized_chat_detour(
        self: *mut Self,
        message: *const FText,
        channel: u32,
    ) {
        let controller = self.cast::<c_void>();
        if let Some(raw) = native_string(message) {
            if display_broadcast(controller, channel, &raw).is_some() {
                return;
            }
        }
        unsafe {
            self.client_receive_localized_chat(message, channel);
        }
    }
}

fn display_parts(channel: u32, message: &str) -> Option<(&str, &str, bool, [f32; 4])> {
    if channel != EChatType::AllSay as u32 && channel != EChatType::ServerSay as u32 {
        return None;
    }

    let (title, body, color) = broadcast_parts(message)?;
    // Presentation only: this hook receives localized system chat, not player commands.
    let is_sender_dev = title == "Server Console";
    let rgb = if is_sender_dev {
        0x00FFFF
    } else {
        color.unwrap_or(0xFFFFFF)
    };
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
    use crate::chivalry2::engine::{UWorld, APlayerController, UObject, UObjectType, UPlayer};

    let (title, body, is_sender_dev, origin_color) = display_parts(channel, raw)?;
    if controller.is_null() {
        return None;
    }
    // Match ClientReceiveLocalizedChat's Player guard and virtual GetWorld call.
    let controller_ptr = controller.cast::<APlayerController>();
    let controller = unsafe { APlayerController::from_ptr(controller) }?;
    unsafe { UPlayer::from_ptr(controller.player) }?;
    let world = unsafe { UWorld::from_ptr(controller.base.get_world()) }?;
    let instance = world.owning_game_instance;
    if !unsafe { UObject::from_ptr(instance.cast())?.inherits_from("TBLGameInstance") } {
        return None;
    }
    let title = InvariantText::new(title);
    let body = InvariantText::new(body);
    // Preflight both calls before producing any output; failures use the untouched native path.
    if !crate::resolvers::function_is_bound("TBLGameInstance_AddChatLine")
        || !unchained_macros::detour_is_initialized("ClientMessage")
    {
        return None;
    }
    let mut console_line = FString::from(format!("<{channel}>: {raw}").as_str());
    // Use the existing capture hook as well as native console output, exactly once.
    unsafe {
        controller_ptr.client_message_detour(&mut console_line, FName::default(), 0.0);
    }
    let [r, g, b, a] = origin_color;
    let origin_color = FLinearColor { r, g, b, a };
    unsafe {
        instance.cast::<UTBLGameInstance>().add_chat_line(
            title.as_ptr(),
            body.as_ptr(),
            channel,
            false,
            is_sender_dev,
            &origin_color,
        );
    }
    Some(())
}

fn native_string(text: *const FText) -> Option<String> {
    if text.is_null() || !crate::resolvers::function_is_bound("FText_ToString") {
        return None;
    }
    let string = unsafe { FText::from_ptr(text)?.to_native_string() };
    string.copy_to_string().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_override_accepts_only_global_channels() {
        let message = "{$Map Change}\nStarting match on Wardenglade.";
        for channel in EChatType::ALL {
            let expected = matches!(channel, EChatType::AllSay | EChatType::ServerSay).then_some((
                "Map Change",
                "Starting match on Wardenglade.",
                false,
                [1.0; 4],
            ));
            assert_eq!(display_parts(channel as u32, message), expected);
        }
        assert!(display_parts(u32::MAX, message).is_none());
    }

    #[test]
    fn null_controller_skips_native_display() {
        assert!(display_broadcast(
            std::ptr::null_mut(),
            EChatType::AllSay as u32,
            "{$Title}\nHello"
        )
        .is_none());
    }

    #[test]
    fn ordinary_chat_does_not_enable_server_console_presentation() {
        assert!(display_parts(EChatType::AllSay as u32, "Server Console: Hello").is_none());
    }

    #[test_case::test_case(EChatType::AllSay, "{$Server Console}\nHello"; "global")]
    #[test_case::test_case(EChatType::ServerSay, "{$Server Console}\nHello"; "server")]
    #[test_case::test_case(EChatType::AllSay, "{$Server Console|#FF0000}\nHello"; "conflicting_color")]
    fn server_console_presentation_is_always_developer_cyan(channel: EChatType, message: &str) {
        assert_eq!(
            display_parts(channel as u32, message),
            Some(("Server Console", "Hello", true, [0.0, 1.0, 1.0, 1.0]))
        );
    }

    #[test]
    fn black_broadcast_color_is_not_replaced_by_default_white() {
        let (_, _, _, color) =
            display_parts(EChatType::AllSay as u32, "{$Title|#000000}\nHello").unwrap();
        assert_eq!(color, [0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn broadcast_rgb_becomes_linear_origin_color_with_opaque_alpha() {
        let (_, body, is_sender_dev, color) =
            display_parts(EChatType::AllSay as u32, "{$Announcement|#FF8000}\nHello").unwrap();
        assert_eq!(body, "Hello");
        assert!(!is_sender_dev);
        assert_eq!([color[0], color[2], color[3]], [1.0, 0.0, 1.0]);
        assert!((color[1] - 0.2158605).abs() < 0.000001);
    }
}
