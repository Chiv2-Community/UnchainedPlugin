//! Observe live print arguments without consuming the VM frame or its owners.
use crate::Chivalry2::Engine::{UKismetSystemLibrary, UObject, FString, FText, FLinearColor};
use crate::unchained::blueprints::print_logging::log_blueprint_print;

#[unchained_macros_core::bind_func_entry_listener(UKismetSystemLibrary::PrintWarning)]
unsafe fn print_warning(message: &FString) {
    if let Ok(message) = message.copy_to_string() {
        log_blueprint_print("PrintWarning", &message);
    }
}

#[unchained_macros_core::bind_func_entry_listener(UKismetSystemLibrary::PrintString)]
unsafe fn print_string(
    _context: &*mut UObject, message: &FString, _print_to_screen: &bool,
    _print_to_log: &bool, _color: &FLinearColor, _duration: &f32,
) {
    if let Ok(message) = message.copy_to_string() {
        log_blueprint_print("PrintString", &message);
    }
}

#[unchained_macros_core::bind_func_entry_listener(UKismetSystemLibrary::PrintText)]
unsafe fn print_text(
    _context: &*mut UObject, message: &FText, _print_to_screen: &bool,
    _print_to_log: &bool, _color: &FLinearColor, _duration: &f32,
) {
    if let Ok(message) = unsafe { message.ToString() }.copy_to_string() {
        log_blueprint_print("PrintText", &message);
    }
}
