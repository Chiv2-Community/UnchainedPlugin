//! Repository-level guard for the native binding/plugin replacement split.
use quote::ToTokens;
use std::{collections::BTreeMap, path::Path};
use syn::{punctuated::Punctuated, Attribute, Expr, ImplItem, Item, Signature, Token};
use syn::visit_mut::VisitMut;

#[derive(Debug, PartialEq, Eq)]
struct Binding {
    resolver: String,
    signature: String,
    cfg: String,
}

fn attribute<'a>(attrs: &'a [Attribute], name: &str) -> Option<&'a Attribute> {
    attrs
        .iter()
        .find(|a| a.path().segments.last().unwrap().ident == name)
}

fn args(attr: &Attribute) -> Vec<Expr> {
    attr.parse_args_with(Punctuated::<Expr, Token![,]>::parse_terminated)
        .unwrap()
        .into_iter()
        .collect()
}

#[test]
fn detour_entry_points_use_rust_parameters_and_precede_supporting_items() {
    fn check(path: &Path) {
        for entry in std::fs::read_dir(path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                check(&path);
                continue;
            }
            if path.extension().is_none_or(|extension| extension != "rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            let file = syn::parse_file(&source).unwrap();
            let mut supporting_item_seen = false;
            for item in file.items {
                match item {
                    Item::Use(_) if !supporting_item_seen => {}
                    Item::Fn(function)
                        if attribute(&function.attrs, "bind_func_detour").is_some() =>
                    {
                        for input in &function.sig.inputs {
                            let syn::FnArg::Typed(arg) = input else { continue };
                            let syn::Pat::Ident(arg) = &*arg.pat else { continue };
                            assert!(
                                arg.ident.to_string().chars().all(|c|
                                    c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                                "{}: detour {} parameter {} must use snake_case",
                                path.display(),
                                function.sig.ident,
                                arg.ident,
                            );
                        }
                        assert!(
                            !supporting_item_seen,
                            "{}: detour {} follows supporting code",
                            path.display(),
                            function.sig.ident,
                        );
                    }
                    _ => supporting_item_seen = true,
                }
            }
        }
    }

    check(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/unchained/patches"));
}

fn signature(sig: &Signature) -> String {
    let mut sig = sig.clone();
    sig.ident = syn::parse_quote!(native);
    sig.inputs.pop_punct();
    sig.to_token_stream().to_string()
}

fn cfg(attrs: &[Attribute]) -> String {
    attrs
        .iter()
        .filter(|a| a.path().is_ident("cfg") || a.path().is_ident("cfg_attr"))
        .map(|a| a.to_token_stream().to_string())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(" ")
}

fn scan(
    path: &Path,
    bindings: &mut BTreeMap<String, Binding>,
    detours: &mut BTreeMap<String, (String, String)>,
) {
    scan_with_cfg(path, bindings, detours, &[]);
}

fn scan_with_cfg(
    path: &Path,
    bindings: &mut BTreeMap<String, Binding>,
    detours: &mut BTreeMap<String, (String, String)>,
    inherited: &[Attribute],
) {
    let index = std::fs::read_to_string(path.join("mod.rs"))
        .ok()
        .map(|source| syn::parse_file(&source).unwrap());
    let mut inherited = inherited.to_vec();
    if let Some(index) = &index {
        inherited.extend(index.attrs.clone());
    }
    for entry in std::fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        let mut module_attrs = inherited.clone();
        let name = if path.is_dir() { path.file_name() } else { path.file_stem() }
            .unwrap().to_str().unwrap();
        if let Some(index) = &index {
            for item in &index.items {
                if let Item::Mod(module) = item {
                    if module.ident == name {
                        module_attrs.extend(module.attrs.clone());
                    }
                }
            }
        }
        if path.is_dir() {
            scan_with_cfg(&path, bindings, detours, &module_attrs);
            continue;
        }
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let file = syn::parse_file(&std::fs::read_to_string(&path).unwrap()).unwrap();
        module_attrs.extend(file.attrs);
        for item in file.items {
            match item {
                Item::Impl(item) => {
                    let owner = item.self_ty.to_token_stream().to_string();
                    let owner = if owner == "TArray < TCHAR >" {
                        "FString"
                    } else {
                        &owner
                    };
                    for member in item.items {
                        let ImplItem::Fn(function) = member else {
                            continue;
                        };
                        assert_no_legacy(&function.attrs, &path);
                        assert!(
                            attribute(&function.attrs, "bind_func_detour").is_none(),
                            "replacement must be a free function: {}",
                            path.display()
                        );
                        for name in ["bind_resolved_func", "bind_resolved_request_function"] {
                            let Some(attr) = attribute(&function.attrs, name) else {
                                continue;
                            };
                            // Only descriptor-enabled bindings can be detached targets.
                            if attribute(&item.attrs, "runtime_bindings").is_none() {
                                continue;
                            }
                            let key = format!("{owner}::{}", function.sig.ident);
                            let resolver = args(attr)[0].to_token_stream().to_string();
                            assert!(
                                bindings
                                    .insert(
                                        key,
                                        Binding {
                                            resolver,
                                            signature: signature(&function.sig),
                                            cfg: cfg(&[
                                                module_attrs.clone(),
                                                item.attrs.clone(),
                                                function.attrs.clone(),
                                            ].concat()),
                                        }
                                    )
                                    .is_none(),
                                "duplicate native binding"
                            );
                            assert!(
                                function.block.stmts.is_empty(),
                                "native declaration contains replacement policy: {}",
                                path.display()
                            );
                        }
                    }
                }
                Item::Fn(function) => {
                    assert_no_legacy(&function.attrs, &path);
                    let Some(attr) = attribute(&function.attrs, "bind_func_detour") else {
                        continue;
                    };
                    assert!(
                        path.components()
                            .any(|part| part.as_os_str() == "unchained"),
                        "replacement belongs outside chivalry2: {}",
                        path.display()
                    );
                    let args = args(attr);
                    let target = args[0].to_token_stream().to_string().replace(' ', "");
                    let condition = args
                        .get(1)
                        .map(|e| e.to_token_stream().to_string())
                        .unwrap_or_else(|| "ACTIVE".into());
                    assert!(
                        detours
                            .insert(target, (condition, cfg(&[
                                module_attrs.clone(),
                                function.attrs.clone(),
                            ].concat())))
                            .is_none(),
                        "duplicate detached target"
                    );
                }
                _ => {}
            }
        }
    }
}

fn assert_no_legacy(attrs: &[Attribute], path: &Path) {
    for name in [
        "bind_resolved_func_detour",
        "bind_resolved_request_function_detour",
    ] {
        assert!(
            attribute(attrs, name).is_none(),
            "legacy detour in {}",
            path.display()
        );
    }
}

#[test]
fn patches_follow_plugin_purposes_not_native_header_paths() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/unchained/patches");
    assert!(!root.with_file_name("detours").exists(), "detours belong in patches");
    for old_root in [
        "engine", "plugins", "tbl", "tbl_loading_screen",
        "Engine", "Plugins", "TBL", "TBLLoadingScreen",
    ] {
        assert!(!root.join(old_root).exists(), "native hierarchy in patches: {old_root}");
    }
    let desync = syn::parse_file(
        &std::fs::read_to_string(root.join("hosting/desync/mod.rs")).unwrap()
    ).unwrap();
    let gated_modules: Vec<_> = desync.items.iter().filter_map(|item| {
        let Item::Mod(module) = item else { return None };
        if cfg(&module.attrs).is_empty() {
            return None;
        }
        Some((module.ident.to_string(), cfg(&module.attrs)))
    }).collect();
    let gate = cfg(&[syn::parse_quote!(#[cfg(feature = "move-autonomous-desync")])]);
    assert_eq!(
        gated_modules,
        [("tick_actors".into(), gate)],
        "legacy movement patches must retain their module feature gates",
    );
    for (path, targets) in [
        ("config_overrides.rs", &["UObject::LoadConfig", "UClass::GetConfigName"][..]),
        ("frontend_patches.rs", &["UTBLGameInstance::LoadFrontEndMap"][..]),
        ("backend_hooks/auth_token_cleanup.rs", &["FOnlineAuthPlayFab::EndAuthentication"][..]),
        ("custom_loading_screens.rs", &["UMapSpecificLoadingScreenConfig::GetDataForMap"][..]),
        ("damage_events.rs", &["ATBLCharacter::OnKilled", "ATBLCharacter::OnDamageTaken"][..]),
        ("hosting/desync/prevent_stackoverflow.rs", &["UObject::ProcessEvent"][..]),
        ("hosting/playable_listen.rs", &["UGameplay::IsDedicatedServer"][..]),
        ("hosting/server_init.rs", &["UGameEngine::DedicatedServerInit"][..]),
        ("hosting/graceful_shutdown.rs", &["UTBLEngine::PreExit"][..]),
        ("ownership_overrides.rs", &[
            "ATBLPlayerController::GetOwnershipFromPlayerControllerAndState",
            "ATBLPlayerController::CanUseLoadoutItem",
            "ATBLPlayerController::CanUseCharacter",
            "ATBLPlayerController::ConditionalInitializeCustomizationOnServer",
        ][..]),
        ("messages/chat.rs", &[
            "ATBLPlayerController::ClientReceiveChat_Implementation",
            "ATBLGameMode::BroadcastChat",
            "APlayerController::ClientMessage",
        ][..]),
        ("networking/travel.rs", &["APlayerController::ClientTravelInternal"][..]),
        ("networking/ipv6_support.rs", &[
            "FUDPPing::UDPEcho",
            "FString::Find",
            "FString::ParseIntoArray",
            "FOnlineSessionPlayFab::OnGetCurrentGames",
            "FOnlineSessionPlayFab::PingSomeServers",
        ][..]),
        ("pak_mount.rs", &["FPakPlatformFile::Mount"][..]),
        ("backend_hooks/server_browser.rs", &[
            "OpenAPIClientAPI::GetPost_GetMotd",
            "OpenAPIClientAPI::GetPost_GetCurrentGames",
            "PlayFabRequestHandler::SendRequest",
        ][..]),
        ("diagnostics.rs", &[
            "UUserFeedbackAndBugReportsLibrary::GetGameInfo",
            "TBLUtility::ExecuteConsoleCommand",
            "UGameplayStatics::LoadGameFromSlot",
            "UGameplayStatics::SaveGameToSlot",
            "FConsoleOutputDevice::Serialize",
            "FFrame::KismetExecutionMessage",
        ][..]),
        ("loose_assets.rs", &[
            "FPakPlatformFile::FindFileInPakFiles",
            "FPakPlatformFile::FindFileInPakFiles_Static",
            "FPakPlatformFile::IsNonPakFilenameAllowed",
        ][..]),
    ] {
        let source = std::fs::read_to_string(root.join(path)).unwrap();
        let file = syn::parse_file(&source).unwrap();
        let actual: Vec<_> = file.items.iter().filter_map(|item| {
            let Item::Fn(function) = item else { return None };
            let attr = attribute(&function.attrs, "bind_func_detour")?;
            Some(args(attr)[0].to_token_stream().to_string().replace(' ', ""))
        }).collect();
        for target in targets {
            assert!(actual.iter().any(|actual| actual == target), "{target} belongs in {path}");
        }
    }
    for (path, expected) in [
        ("enable_console_commands.rs", "UTBLLocalPlayer__Exec, BYTES, &[0xEB]"),
        ("hosting/connections.rs", "AllowAlternateBackend, 0x4D, BYTES, &[0x90, 0xE9]"),
        ("hosting/desync/net_mode.rs", "UNetDriver__GetNetMode, BYTES, &[0xB8, 0x01, 0x00, 0x00, 0x00, 0xC3], IF { cli_args().apply_desync_patch }"),
        ("hosting/desync/tick_actors.rs", "MoveAutonomous, 0x2f6, BYTES, &[0xE9, 0xBE, 0x0, 0x0, 0x0, 0x90], IF { cli_args().tick_actor_patch }"),
        ("hosting/desync/tick_actors.rs", "TickActor, 0x73, NOP, 6, IF { cli_args().tick_actor_patch }"),
        ("enable_console_commands.rs", "AuthenticateConsoleCommandCaller, BYTES, &[0xEB]"),
        ("backend_hooks/auth_token_cleanup.rs", "FLogUserIntoOnlineSubsystemNode__GetAuthTokenPatchSite, BYTES, &[0xEB, 0x26]"),
        ("frontend_patches.rs", "EACAntiCheatMesssage, 0xE, NOP, 5"),
    ] {
        let file = syn::parse_file(&std::fs::read_to_string(root.join(path)).unwrap()).unwrap();
        let actual: Vec<_> = file.items.iter().filter_map(|item| {
            let Item::Macro(item) = item else { return None };
            item.mac.path.is_ident("CREATE_PATCH").then(|| item.mac.tokens.to_string())
        }).collect();
        let expected = expected.parse::<proc_macro2::TokenStream>().unwrap().to_string();
        assert_eq!(
            actual.iter().filter(|tokens| **tokens == expected).count(),
            1,
            "instruction patch changed, duplicated, or misplaced: {path}",
        );
    }
}

#[test]
fn native_binding_and_resolver_definitions_stay_outside_unchained() {
    struct NoNativeBindings<'a>(&'a Path);
    impl VisitMut for NoNativeBindings<'_> {
        fn visit_attribute_mut(&mut self, attr: &mut Attribute) {
            let name = attr.path().segments.last().unwrap().ident.to_string();
            assert!(
                ![
                    "runtime_bindings",
                    "bind_resolved_func",
                    "bind_resolved_request_function",
                    "bind_resolver_value",
                ]
                .contains(&name.as_str()),
                "native binding belongs outside unchained: {}",
                self.0.display()
            );
            syn::visit_mut::visit_attribute_mut(self, attr);
        }

        fn visit_macro_mut(&mut self, mac: &mut syn::Macro) {
            let name = mac.path.segments.last().unwrap().ident.to_string();
            assert!(
                !(name.starts_with("define_") && name.ends_with("resolver")
                    || name == "impl_resolver_singleton"),
                "native resolver belongs outside unchained: {}",
                self.0.display()
            );
            syn::visit_mut::visit_macro_mut(self, mac);
        }
    }

    fn check(directory: &Path) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                check(&path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let source = std::fs::read_to_string(&path).unwrap();
                let mut file = syn::parse_file(&source).unwrap();
                NoNativeBindings(&path).visit_file_mut(&mut file);
            }
        }
    }
    check(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/unchained"));
}

#[test]
fn native_callables_use_cpp_style_names() {
    struct CheckNames<'a>(&'a Path);

    impl CheckNames<'_> {
        fn check(&self, name: &syn::Ident) {
            assert!(
                name.to_string().starts_with(|c: char| c.is_ascii_uppercase()),
                "{}: native callable {name} must use C++ spelling",
                self.0.display(),
            );
        }

        fn binding(&self, attrs: &[Attribute], signature: &Signature) {
            if ["bind_resolved_func", "bind_resolved_request_function"]
                .iter().any(|name| attribute(attrs, name).is_some())
            {
                self.check(&signature.ident);
            }
        }
    }

    impl VisitMut for CheckNames<'_> {
        fn visit_ident_mut(&mut self, name: &mut syn::Ident) {
            assert!(
                !name.to_string().contains("Tbl"),
                "{}: {name} must preserve TBL capitalization",
                self.0.display(),
            );
        }

        fn visit_item_fn_mut(&mut self, item: &mut syn::ItemFn) {
            self.binding(&item.attrs, &item.sig);
            syn::visit_mut::visit_item_fn_mut(self, item);
        }

        fn visit_impl_item_fn_mut(&mut self, item: &mut syn::ImplItemFn) {
            self.binding(&item.attrs, &item.sig);
            syn::visit_mut::visit_impl_item_fn_mut(self, item);
        }

        fn visit_attribute_mut(&mut self, attr: &mut Attribute) {
            if attr.path().is_ident("vtable") {
                let mut tokens = attr.meta.require_list().unwrap().tokens.clone()
                    .into_iter().peekable();
                while let Some(token) = tokens.next() {
                    if let proc_macro2::TokenTree::Ident(name) = token {
                        if name != "offset" && matches!(
                            tokens.peek(),
                            Some(proc_macro2::TokenTree::Group(group))
                                if group.delimiter() == proc_macro2::Delimiter::Parenthesis
                        ) {
                            self.check(&name);
                        }
                    }
                }
            }
            syn::visit_mut::visit_attribute_mut(self, attr);
        }
    }

    fn check(directory: &Path) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                check(&path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let mut file = syn::parse_file(&std::fs::read_to_string(&path).unwrap()).unwrap();
                CheckNames(&path).visit_file_mut(&mut file);
            }
        }
    }

    check(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/chivalry2"));
}

#[test]
fn project_binding_resolvers_match_their_targets() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/chivalry2");
    let mut bindings = BTreeMap::new();
    scan(&root, &mut bindings, &mut BTreeMap::new());
    for (target, binding) in bindings {
        let resolver: syn::Path = syn::parse_str(&binding.resolver).unwrap();
        if resolver.segments.first().unwrap().ident == "patternsleuth" {
            continue;
        }
        let name = resolver.segments.last().unwrap().ident.to_string();
        let (owner, method) = target.rsplit_once("::").unwrap();
        assert!(
            name == method || name == format!("{owner}__{method}"),
            "{target}: resolver {name} must be Method or Class__Method",
        );
        assert!(!name.contains("Tbl"), "{name} must preserve TBL capitalization");
    }
}

#[test]
fn native_modules_preserve_header_casing() {
    fn check(directory: &Path) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                check(&path);
                continue;
            }
            if path.extension().is_none_or(|extension| extension != "rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            if path.file_name().unwrap() != "mod.rs" {
                let header = source.lines().next().unwrap().split_once(".h").unwrap().0;
                let basename = header.rsplit('/').next().unwrap();
                assert_eq!(path.file_stem().unwrap(), basename, "{}", path.display());
            }
            let file = syn::parse_file(&source).unwrap();
            for item in file.items {
                let Item::Mod(module) = item else { continue };
                if module.content.is_some() {
                    continue;
                }
                let name = module.ident.to_string();
                assert!(
                    directory.join(format!("{name}.rs")).is_file()
                        || directory.join(&name).join("mod.rs").is_file(),
                    "{}: module {name} must match its native file's casing",
                    path.display(),
                );
            }
        }
    }
    check(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/chivalry2"));
}

#[test]
fn engine_bindings_preserve_native_source_categories() {
    fn check(root: &Path, directory: &Path) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                check(root, &path);
                continue;
            }
            if path.extension().is_none_or(|extension| extension != "rs")
                || path.file_name().unwrap() == "mod.rs"
            {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            let header = source.lines().next().unwrap();
            let category = if header.contains("Engine/Source/Runtime/") {
                "Runtime"
            } else if header.contains("Engine/Source/ThirdParty/") {
                "ThirdParty"
            } else if header.contains("Engine/Plugins/") {
                "Plugins"
            } else {
                panic!("missing native source category: {}", path.display());
            };
            assert!(
                path.strip_prefix(root).unwrap().starts_with(category),
                "native source category mismatch: {}",
                path.display()
            );
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/chivalry2/Engine");
    check(&root, &root);
}

#[test]
fn custom_blueprints_and_patch_policy_stay_outside_chivalry2() {
    struct NoPluginPolicy<'a>(&'a Path);
    impl VisitMut for NoPluginPolicy<'_> {
        fn visit_item_macro_mut(&mut self, item: &mut syn::ItemMacro) {
            assert!(
                !item.mac.path.segments.last().is_some_and(|segment|
                    ["CREATE_PATCH", "CREATE_HOOK"].contains(&segment.ident.to_string().as_str())
                ),
                "plugin patch/hook policy belongs outside chivalry2: {}",
                self.0.display()
            );
            syn::visit_mut::visit_item_macro_mut(self, item);
        }
    }

    fn check(directory: &Path) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                check(&path);
                continue;
            }
            if path.extension().is_none_or(|extension| extension != "rs") {
                continue;
            }
            let mut file = syn::parse_file(&std::fs::read_to_string(&path).unwrap()).unwrap();
            NoPluginPolicy(&path).visit_file_mut(&mut file);
            for item in file.items {
                match item {
                    Item::Struct(item) => assert!(
                        ![
                            "DA_ModMarker_C",
                            "DA_ModMarker_V2_C",
                            "ArgonSDKModBase",
                            "UModLoaderSettings_C",
                        ]
                        .contains(&item.ident.to_string().as_str()),
                        "custom/private model belongs in unchained: {}",
                        path.display()
                    ),
                    Item::Mod(item) => assert!(
                        !["extensions", "startup_patch", "backend", "chat"]
                            .contains(&item.ident.to_string().as_str()),
                        "non-header module belongs outside chivalry2: {}",
                        path.display()
                    ),
                    Item::Use(item) if !matches!(item.vis, syn::Visibility::Inherited) => assert!(
                        !item
                            .tree
                            .to_token_stream()
                            .to_string()
                            .contains("unchained"),
                        "native indexes must not re-export private integrations: {}",
                        path.display()
                    ),
                    _ => {}
                }
            }
        }
    }
    check(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/chivalry2"));
}

#[test]
fn tbl_declarations_follow_original_native_header_owners() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/chivalry2");
    for (path, names) in [
        ("TBL/PlayerTypes.rs", &["EChatType", "EFaction"][..]),
        ("TBL/TBLTypes.rs", &["FDamageTakenEvent", "UGameplay"][..]),
        (
            "TBL/TBLPlayerController.rs",
            &["ATBLPlayerController", "FOwnershipResponse"][..],
        ),
        ("TBL/Abilities/DamageSource.rs", &["UDamageSource"][..]),
        ("TBL/Game/TBLGameState.rs", &["ATBLGameState"][..]),
        ("TBL/Online/TBLOnlineStats.rs", &["FTBLOnlineStats"][..]),
        ("TBL/TBLUtilityLibrary.rs", &["TBLUtility"][..]),
        (
            "TBL/UI/UserFeedbackAndBugReportsLibrary.rs",
            &["UUserFeedbackAndBugReportsLibrary"][..],
        ),
        (
            "TBLLoadingScreen/MapSpecificLoadingScreenConfig.rs",
            &["UMapSpecificLoadingScreenConfig", "FMapSpecificLoadingScreenConfigData"][..],
        ),
    ] {
        let source = std::fs::read_to_string(root.join(path)).unwrap();
        let file = syn::parse_file(&source).unwrap();
        for name in names {
            assert!(file.items.iter().any(|item| match item {
                Item::Struct(item) => item.ident == *name,
                Item::Enum(item) => item.ident == *name,
                _ => false,
            }), "{name} belongs in {path}");
        }
    }
    // Source-owner directories are legitimate native modules, not plugin policies.
    for path in ["TBL/Online/mod.rs", "TBL/UI/mod.rs"] {
        assert!(root.join(path).is_file());
    }
}

#[test]
fn playfab_native_types_follow_header_owners() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../src/chivalry2/Plugins/OnlineSubsystemPlayFab");
    for (path, name) in [
        ("OnlineAuthInterfacePlayFab.rs", "FOnlineAuthPlayFab"),
        (
            "OnlineSessionInterfacePlayFab.rs",
            "FOnlineSessionPlayFab",
        ),
        ("OpenAPIMerged/OpenAPIClientApi.rs", "OpenAPIClientAPI"),
        (
            "PlayFabCpp/Core/PlayFabResultHandler.rs",
            "PlayFabRequestHandler",
        ),
    ] {
        let source = std::fs::read_to_string(root.join(path)).unwrap();
        let file = syn::parse_file(&source).unwrap();
        assert!(file.items.iter().any(|item| matches!(
            item, Item::Struct(item) if item.ident == name
        )));
        assert!(!source.contains("CREATE_PATCH!"));
    }
}

#[test]
fn redpoint_instruction_resolver_follows_authentication_node_owner() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src");
    let source = std::fs::read_to_string(root.join(
        "chivalry2/Plugins/OnlineSubsystemRedpointEOS/Shared/Authentication/Nodes/LogUserIntoOnlineSubsystemNode.rs",
    )).unwrap();
    let file = syn::parse_file(&source).unwrap();
    assert!(file.items.iter().any(|item| matches!(
        item, Item::Macro(item)
            if item.mac.path.is_ident("define_single_platform_pattern_resolver")
                && item.mac.tokens.to_string().starts_with("STEAM , FLogUserIntoOnlineSubsystemNode__GetAuthTokenPatchSite ,")
    )));
    assert!(!source.contains("CREATE_PATCH!"));
}

#[test]
fn migrated_detours_preserve_native_signatures_conditions_and_feature_gates() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src");
    let (mut bindings, mut detours) = (BTreeMap::new(), BTreeMap::new());
    scan(&root, &mut bindings, &mut detours);
    let baseline = include_str!("native_detours.tsv");
    let mut expected = 0;
    for line in baseline
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let fields: Vec<_> = line.split('\t').collect();
        assert_eq!(fields.len(), 5, "invalid baseline row");
        let [target, resolver, sig, condition, gate] = fields.as_slice() else {
            unreachable!()
        };
        let binding = bindings
            .get(*target)
            .unwrap_or_else(|| panic!("missing binding: {target}"));
        let sig: Signature = syn::parse_str(sig).unwrap();
        let resolver: Expr = syn::parse_str(resolver).unwrap();
        assert_eq!(
            binding,
            &Binding {
                resolver: resolver.to_token_stream().to_string(),
                signature: signature(&sig),
                cfg: gate.to_string(),
            },
            "{target}"
        );
        let condition: Expr = syn::parse_str(condition).unwrap();
        assert_eq!(
            detours.remove(*target),
            Some((condition.to_token_stream().to_string(), gate.to_string())),
            "{target}"
        );
        expected += 1;
    }
    assert_eq!(expected, 45);
    assert!(
        detours.is_empty(),
        "unreviewed detached targets: {detours:?}"
    );
}
