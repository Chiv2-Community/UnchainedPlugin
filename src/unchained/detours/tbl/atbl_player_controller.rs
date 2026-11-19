//! Plugin replacements for `crate::chivalry2::tbl::atbl_player_controller`.

use crate::chivalry2::engine::FLinearColor;
use crate::chivalry2::engine::FName;
use crate::chivalry2::engine::FPrimaryAssetId;
use crate::chivalry2::engine::FString;
use crate::chivalry2::engine::FText;
use crate::chivalry2::engine::InvariantText;
use crate::chivalry2::engine::UObjectType;
use crate::chivalry2::engine::TSubclassOfAInventoryItem;
use crate::chivalry2::engine::TSubclassOfATBLCharacter;
use crate::chivalry2::engine::TSubclassOfULoadoutSelection;
use crate::chivalry2::tbl::atbl_player_controller::*;
use crate::chivalry2::tbl::fownership_response::FOwnershipResponse;
use crate::chivalry2::tbl::{ATBLPlayerState, EChatType, UTBLGameInstance};
use crate::events::broadcast_message::game_chat::broadcast_parts;
use crate::unchained::detours::engine::runtime::engine::game_framework::player_controller::{
    capture_chat, has_authority,
};
use std::os::raw::c_void;

#[unchained_macros::bind_func_detour(ATBLPlayerController::client_receive_chat)]
pub(crate) unsafe fn client_receive_chat_detour(
    this: *mut ATBLPlayerController,
    sender: *mut ATBLPlayerState,
    message: *const FString,
    chat_type: u32,
    is_sender_dev: bool,
    override_color: u32,
) {
    // Hosts capture once at BroadcastChat, not once per local recipient.
    if !has_authority() {
        if let Some(player) = unsafe { ATBLPlayerState::from_ptr(sender) } {
            capture_chat(player, message, chat_type);
        }
    }
    unsafe {
        this.client_receive_chat(sender, message, chat_type, is_sender_dev, override_color);
    }
}

#[unchained_macros::bind_func_detour(
    ATBLPlayerController::get_ownership_from_player_controller_and_state
)]
pub(crate) unsafe fn get_ownership_from_player_controller_and_state_detour(
    result: *mut FOwnershipResponse,
    PlayerController: *mut ATBLPlayerController,
    PlayerState: *mut ATBLPlayerState,
    AssetIdToCheck: *mut FPrimaryAssetId,
    BaseOnly: bool,
) -> *mut FOwnershipResponse {
    // Caller-provided result storage precedes the controller in the native ABI.
    let response = unsafe {
        ATBLPlayerController::get_ownership_from_player_controller_and_state(
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

#[unchained_macros::bind_func_detour(ATBLPlayerController::can_use_loadout_item)]
pub(crate) unsafe fn can_use_loadout_item_detour(
    this: *mut ATBLPlayerController,
    result: *mut FOwnershipResponse,
    InLoadOutSelection: *mut TSubclassOfULoadoutSelection,
    InItem: *mut TSubclassOfAInventoryItem,
) -> *mut FOwnershipResponse {
    let response = unsafe { this.can_use_loadout_item(result, InLoadOutSelection, InItem) };
    if let Some(resp) = unsafe { response.as_mut() } {
        resp.level = 0;
        resp.owned = true;
    }
    if let Some(res) = unsafe { result.as_mut() } {
        res.owned = true;
    }
    response
}

#[unchained_macros::bind_func_detour(ATBLPlayerController::can_use_character)]
pub(crate) unsafe fn can_use_character_detour(
    this: *mut ATBLPlayerController,
    result: *mut FOwnershipResponse,
    CharacterSubclass: *mut TSubclassOfATBLCharacter,
) -> *mut FOwnershipResponse {
    let response = unsafe { this.can_use_character(result, CharacterSubclass) };
    if let Some(resp) = unsafe { response.as_mut() } {
        resp.level = 0;
        resp.owned = true;
    }
    response
}

#[unchained_macros::bind_func_detour(
    ATBLPlayerController::conditional_initialize_customization_on_server
)]
pub(crate) unsafe fn conditional_initialize_customization_on_server_detour(
    this: *mut ATBLPlayerController,
    player_state: *mut c_void,
) -> c_void {
    let this = this;
    if let Some(pc) = unsafe { this.as_mut() } {
        pc.bOnlineInventoryInitialized = true;
        pc.bPlayerCustomizationReceived = true;
    }
    unsafe { this.conditional_initialize_customization_on_server(player_state) }
}

#[unchained_macros::bind_func_detour(ATBLPlayerController::client_receive_localized_chat)]
pub(crate) unsafe fn client_receive_localized_chat_detour(
    this: *mut ATBLPlayerController,
    message: *const FText,
    channel: u32,
) {
    let controller = this.cast::<c_void>();
    if let Some(raw) = native_string(message) {
        if display_broadcast(controller, channel, &raw).is_some() {
            return;
        }
    }
    unsafe {
        this.client_receive_localized_chat(message, channel);
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
    use crate::chivalry2::engine::APlayerController;
    use crate::chivalry2::engine::UObject;
    use crate::chivalry2::engine::UObjectType;
    use crate::chivalry2::engine::UPlayer;
    use crate::chivalry2::engine::UWorld;

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
        crate::unchained::detours::engine::runtime::engine::game_framework::player_controller::client_message_detour(controller_ptr, &mut console_line, FName::default(), 0.0);
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
    fn chat_replacement_uses_tbl_controller_receiver() {
        let _: unsafe fn(
            *mut ATBLPlayerController,
            *mut ATBLPlayerState,
            *const FString,
            u32,
            bool,
            u32,
        ) = client_receive_chat_detour;
    }

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
