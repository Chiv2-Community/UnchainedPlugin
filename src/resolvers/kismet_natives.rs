//! Blueprint VM native-exec hooks on `UKismetSystemLibrary` (print diagnostics + server-identity
//! overrides), plus the Blueprint print logging + spam filter.
//!
//! Shipping folds `UKismetSystemLibrary::PrintString` (and friends) into a shared no-op, so we hook
//! the uniquely registered exec thunks instead. The bindings look up live `UFunction`s by name
//! and attach through `LateHookRegistration` once Unreal reflection is available.
//!
//! `observe` hooks step the real frame once per declared argument, run P_FINISH, and DO NOT call the
//! original (folded no-op); never copy the frame or read an argument out of order. `forward` hooks
//! call the original (which consumes the frame) and may then override `*result`. See
//! docs/blueprint-vm-arg-reading.md and docs/kismet-exec-hooks.md.
//!
//! Split into its own file (out of `messages.rs`) so it transfers cleanly onto
//! `refactor/unchained-macros`; see docs/kismet-handoff.md.

#[allow(unused_imports)]
use crate::ue::{FString, UObject};

kismet_native_class!(UKismetSystemLibrary => path "/Script/Engine.KismetSystemLibrary");

// Other kismet libraries can be bound the same way, e.g.:
// kismet_native_class!(UKismetMathLibrary => path "/Script/Engine.KismetMathLibrary");
// kismet_native_class!(UTBLSystemLibrary => path "/Script/TBL.TBLSystemLibrary");

/// Shared emit for all Blueprint print-family observers (PrintString/PrintWarning/PrintText).
/// Suppresses exact messages listed in the spam filter, then logs to the `blueprint_log` target
/// (console + `blueprint.log`, mirroring the chat log). `kind` is the reflected function name.
#[cfg(feature = "blueprint_printstring_log")]
fn log_blueprint_print(kind: &str, message: &str) {
    if blueprint_spam_filter().contains(message) {
        return;
    }
    sinfo!(target: "blueprint_log", kind = kind; "{}", message);
}

/// Exact full-message strings to suppress, loaded once from `blueprint_filter.txt` in the log
/// directory (one message per line; blank lines and `#` comments ignored). Edit the file and
/// restart the game to change it.
#[cfg(feature = "blueprint_printstring_log")]
fn blueprint_spam_filter() -> &'static std::collections::HashSet<String> {
    use std::sync::OnceLock;
    static FILTER: OnceLock<std::collections::HashSet<String>> = OnceLock::new();
    FILTER.get_or_init(|| {
        let path = crate::logging::log4rs::plugin_log_dir().join("blueprint_filter.txt");
        let set: std::collections::HashSet<String> = std::fs::read_to_string(&path)
            .map(|text| {
                text.lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty() && !line.starts_with('#'))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        sinfo!(
            "[Blueprint] loaded {} spam-filter entrie(s) from {}",
            set.len(),
            path.display()
        );
        set
    })
}

// UKismetSystemLibrary::PrintString(UObject* WorldContextObject, FString InString,
//   bool bPrintToScreen, bool bPrintToLog, FLinearColor TextColor, float Duration)  [UE 4.25]
#[cfg(feature = "blueprint_printstring_log")]
kismet_exec_observe!(UKismetSystemLibrary::PrintString => exec_print_string(
    _world: *mut UObject, message: FString,
    _to_screen: bool, _to_log: bool, _color: [f32; 4], _duration: f32,
) {
    if let Ok(s) = message.copy_to_string() {
        log_blueprint_print("PrintString", &s);
    }
});

// UKismetSystemLibrary::PrintWarning(FString InString)  [UE 4.25] — no WorldContextObject.
#[cfg(feature = "blueprint_printstring_log")]
kismet_exec_observe!(UKismetSystemLibrary::PrintWarning => exec_print_warning(
    message: FString,
) {
    if let Ok(s) = message.copy_to_string() {
        log_blueprint_print("PrintWarning", &s);
    }
});

// UKismetSystemLibrary::PrintText(UObject* WorldContextObject, FText InText,
//   bool bPrintToScreen, bool bPrintToLog, FLinearColor TextColor, float Duration)  [UE 4.25]
#[cfg(feature = "blueprint_printstring_log")]
kismet_exec_observe!(UKismetSystemLibrary::PrintText => exec_print_text(
    _world: *mut UObject, text: FText,
    _to_screen: bool, _to_log: bool, _color: [f32; 4], _duration: f32,
) {
    if let Some(s) = text.to_string() {
        log_blueprint_print("PrintText", &s);
    }
});

// Reflected predicates: exec wrappers with an out-result pointer. With only
// kismet_server_identity_hooks they preserve the engine result; the individual force_* features
// overwrite it. They are kept separate so a build cannot force inconsistent answers.
#[cfg(feature = "kismet_server_identity_hooks")]
kismet_exec_forward!(UKismetSystemLibrary::IsDedicatedServer => exec_is_dedicated_server {
    #[cfg(feature = "force_kismet_is_dedicated_server")]
    if !result.is_null() { *result.cast::<bool>() = true; }
});

#[cfg(feature = "kismet_server_identity_hooks")]
kismet_exec_forward!(UKismetSystemLibrary::IsServer => exec_is_server {
    #[cfg(feature = "force_kismet_is_server")]
    if !result.is_null() { *result.cast::<bool>() = true; }
});

#[cfg(feature = "kismet_server_identity_hooks")]
kismet_exec_forward!(UKismetSystemLibrary::IsStandalone => exec_is_standalone {
    #[cfg(feature = "force_kismet_is_standalone")]
    if !result.is_null() { *result.cast::<bool>() = true; }
});
