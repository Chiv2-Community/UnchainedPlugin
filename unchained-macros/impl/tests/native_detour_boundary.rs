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
        .collect::<Vec<_>>()
        .join(" ")
}

fn scan(
    path: &Path,
    bindings: &mut BTreeMap<String, Binding>,
    detours: &mut BTreeMap<String, (String, String)>,
) {
    for entry in std::fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            scan(&path, bindings, detours);
            continue;
        }
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let file = syn::parse_file(&std::fs::read_to_string(&path).unwrap()).unwrap();
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
                                            cfg: cfg(&function.attrs),
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
                            .insert(target, (condition, cfg(&function.attrs)))
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
fn detours_follow_plugin_purposes_not_native_header_paths() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/unchained/detours");
    for old_root in ["engine", "plugins", "tbl", "tbl_loading_screen"] {
        assert!(!root.join(old_root).exists(), "native hierarchy in detours: {old_root}");
    }
    for (path, targets) in [
        ("config_overrides.rs", &["UObject::load_config", "UClass::get_config_name"][..]),
        ("desync/blueprint_events.rs", &["UObject::process_event"][..]),
        ("ownership_overrides.rs", &[
            "ATBLPlayerController::get_ownership_from_player_controller_and_state",
            "ATBLPlayerController::can_use_loadout_item",
            "ATBLPlayerController::can_use_character",
            "ATBLPlayerController::conditional_initialize_customization_on_server",
        ][..]),
        ("messages/chat.rs", &[
            "ATBLPlayerController::client_receive_chat",
            "ATBLGameMode::broadcast_chat",
            "APlayerController::client_message",
        ][..]),
        ("networking/travel.rs", &["APlayerController::client_travel_internal"][..]),
        ("pak_mount.rs", &["FPakPlatformFile::mount"][..]),
        ("asset_loading/loose_assets.rs", &[
            "FPakPlatformFile::find_file_in_pak_files",
            "FPakPlatformFile::find_file_in_pak_files_static",
            "FPakPlatformFile::is_non_pak_filename_allowed",
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
                "runtime"
            } else if header.contains("Engine/Source/ThirdParty/") {
                "third_party"
            } else if header.contains("Engine/Plugins/") {
                "plugins"
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
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src/chivalry2/engine");
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
        ("tbl/player_types.rs", &["EChatType", "EFaction"][..]),
        ("tbl/tbl_types.rs", &["FDamageTakenEvent", "UGameplay"][..]),
        (
            "tbl/tbl_player_controller.rs",
            &["ATBLPlayerController", "FOwnershipResponse"][..],
        ),
        ("tbl/abilities/damage_source.rs", &["UDamageSource"][..]),
        ("tbl/game/tbl_game_state.rs", &["ATBLGameState"][..]),
        ("tbl/online/tbl_online_stats.rs", &["FTBLOnlineStats"][..]),
        ("tbl/tbl_utility_library.rs", &["TBLUtility"][..]),
        (
            "tbl/ui/user_feedback_and_bug_reports_library.rs",
            &["UUserFeedbackAndBugReportsLibrary"][..],
        ),
        (
            "tbl_loading_screen/map_specific_loading_screen_config.rs",
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
    for path in ["tbl/online/mod.rs", "tbl/ui/mod.rs"] {
        assert!(root.join(path).is_file());
    }
}

#[test]
fn playfab_native_types_follow_header_owners() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../src/chivalry2/plugins/online_subsystem_playfab");
    for (path, name) in [
        ("online_auth_interface_playfab.rs", "FOnlineAuthPlayFab"),
        (
            "online_session_interface_playfab.rs",
            "FOnlineSessionPlayFab",
        ),
        ("open_api_merged/open_api_client_api.rs", "OpenAPIClientAPI"),
        (
            "playfab_cpp/core/playfab_result_handler.rs",
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
