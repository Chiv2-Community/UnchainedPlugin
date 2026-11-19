//! UE 4.25-plus: request handles use [TSharedPtr](https://github.com/EpicGames/UnrealEngine/blob/4.25-plus/Engine/Source/Runtime/Core/Public/Templates/SharedPointer.h).
//! Private OpenAPIClientAPI request ABI; these declarations are not shipped in ArgonSDK.

use crate::chivalry2::engine::FString;
use crate::chivalry2::engine::TSharedPtrIHttpRequest;
use binary_bloodhound::{BinaryMatcher, ExactlyOne, HasAddress, Matcher};
use patternsleuth::MemoryTrait;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::LazyLock;
use std::sync::RwLock;

// OpenAPIClientAPI::CreateHttpRequest(OpenAPIClientAPI *this,TSharedRef<> *__return_storage_ptr__,Request *param_1)
define_custom_resolver!(CreateHttpRequest, {
    OpenAPIClientAPI::request_functions()?.map(|resolved| {
        let (address, offsets) = resolved.value.as_ref();
        sinfo!("Request string → root map:");
        for (name, offset) in offsets {
            sinfo!("  '{}' => 0x{:x}", name, offset);
        }
        *STRING_TO_ROOT_FN
            .write()
            .expect("STRING_TO_ROOT_FN poisoned") = offsets.clone();
        *address
    })
});

/// Common native prefix for Get*Post request parameters.
#[repr(C)]
#[derive(Debug)]
pub struct GenericRequest {
    _private: [u8; 0xD8],
    pub token: FString,
}

#[repr(C)]
#[derive(Debug)]
pub struct OpenAPIClientAPI {
    pub url_base: FString,
}

#[unchained_macros::runtime_bindings]
impl OpenAPIClientAPI {
    /// # Safety
    /// Arguments must satisfy the request's native ABI and game-thread lifetime rules.
    #[unchained_macros::bind_resolved_request_function(GetPost_GetMotd)]
    pub unsafe fn get_post_get_motd(
        self: *mut Self,
        result: *mut TSharedPtrIHttpRequest,
        request: *mut GenericRequest,
        delegate: *mut c_void,
    ) -> *mut TSharedPtrIHttpRequest {
    }

    /// # Safety
    /// Arguments must satisfy the request's native ABI and game-thread lifetime rules.
    #[unchained_macros::bind_resolved_request_function(GetPost_GetCurrentGames)]
    pub unsafe fn get_post_get_current_games(
        self: *mut Self,
        result: *mut TSharedPtrIHttpRequest,
        request: *mut GenericRequest,
        delegate: *mut c_void,
    ) -> *mut TSharedPtrIHttpRequest {
    }

    // Other known request keys; no bindings or hooks are installed for these.
    // GetPost_GetPlayerInventory
    // GetPost_LoginPlayFab
    // GetPost_GetCampaigns
    // GetPost_ReAuthServerCustomId
    // GetPost_PostLogin
    // GetPost_Heartbeat
    // GetPost_PreRegisterGame
    // GetPost_GetMyProgress
    // GetPost_GetJoinTicket
    // GetPost_DeregisterGame
    // GetPost_NotifyPlayerJoined
    // GetPost_GrantFlavorEntitlement
    // GetPost_ReportTelemetry
    // GetPost_PhonebookSearchId
    // GetPost_PhonebookGetOwnId
    // GetPost_Award
    // GetPost_RedeemEntitlementEOS
    // GetPost_GrantPendingRewards
    // GetPost_PurchaseWithVirtualCurrency
    // GetPost_GetTitleData
    // GetPost_LoginEOS
    // GetPost_ConfirmJoinTicket
    // GetPost_PhonebookDeleteOwnId

    fn request_functions() -> patternsleuth::resolvers::Result<
        Matcher<'static, (), RequestFunctions, ExactlyOne, impl Send + Sync>,
    > {
        Ok(BinaryMatcher::patterns([
            "40 53 55 56 41 56 48 83 EC 78 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 44 24",
        ])?
        // Preserve the original resolver's first-match selection.
        .first()
        .flat_map(|request| {
            let address = *request.value;
            BinaryMatcher::address(address)
                .callers()
                .root_functions()
                .with_context()
                .map_filter(|(ctx, root)| {
                    let call = root.parent().address();
                    // Target-build LEA: its displacement starts 0x4c past the call.
                    let displacement = call.checked_add(0x49 + 0x3)?;
                    let memory = &ctx.image().memory;
                    let url = memory.read_wstring(memory.rip4(displacement).ok()?).ok()?;
                    let endpoint = url.rsplit('/').next()?;
                    let offset = root.address().checked_sub(ctx.image().base_address)?;
                    Some((format!("GetPost_{endpoint}"), offset))
                })
                // Keep each endpoint paired with its own caller's function entry.
                .transform(move |functions| {
                    let offsets = functions
                        .iter()
                        .map(|node| node.value.as_ref().clone())
                        .collect();
                    (address, offsets)
                })
        })
        .ensure_one())
    }
}

pub static STRING_TO_ROOT_FN: LazyLock<RwLock<HashMap<String, u64>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

inventory::submit! {
    crate::resolvers::OffsetRegisty {
        name: "CreateHttpRequest Get*Post hooks",
        map: || STRING_TO_ROOT_FN.read().unwrap().clone(),
    }
}

type RequestFunctions = (u64, HashMap<String, u64>);

#[cfg(test)]
mod tests {
    use super::*;
    use ::object::SectionKind;
    use patternsleuth::{
        image::{pe::PEImage, Image, ImageType},
        Memory, NamedMemorySection,
    };
    use std::collections::BTreeMap;

    const BASE: u64 = 0x1_4000_0000;
    const PROLOGUE: [u8; 24] = [
        0x40, 0x53, 0x55, 0x56, 0x41, 0x56, 0x48, 0x83, 0xEC, 0x78, 0x48, 0x8B, 0x05, 0, 0, 0, 0,
        0x48, 0x33, 0xC4, 0x48, 0x89, 0x44, 0x24,
    ];

    #[test]
    fn only_functional_request_hooks_are_registered() {
        type RequestFunction = unsafe fn(
            *mut OpenAPIClientAPI,
            *mut TSharedPtrIHttpRequest,
            *mut GenericRequest,
            *mut c_void,
        ) -> *mut TSharedPtrIHttpRequest;
        let _: RequestFunction = OpenAPIClientAPI::get_post_get_motd;
        let _: RequestFunction = OpenAPIClientAPI::get_post_get_current_games;
        let _: RequestFunction =
            crate::unchained::detours::integrations::open_api_client_api::get_post_get_motd_detour;
        let mut keys: Vec<_> = inventory::iter::<crate::resolvers::HookRegistration>
            .into_iter()
            .map(|registration| registration.name)
            .filter(|name| name.starts_with("GetPost_"))
            .collect();
        keys.sort_unstable();
        assert_eq!(keys, ["GetPost_GetCurrentGames", "GetPost_GetMotd"]);
    }

    #[test]
    fn invalid_request_pointers_do_not_call_native_code_or_prepare_overrides() {
        let client = std::ptr::null_mut::<OpenAPIClientAPI>();
        let request = std::ptr::null_mut::<GenericRequest>();
        let result = unsafe {
            crate::unchained::detours::integrations::open_api_client_api::with_backend_request(
                client,
                request,
                || panic!("must not call native code"),
            )
        };
        assert!(result.is_null());
    }

    struct Fixture {
        text: Vec<u8>,
        strings: Vec<u8>,
        functions: BTreeMap<u32, u32>,
    }

    impl Fixture {
        fn new() -> Self {
            let mut fixture = Self {
                text: vec![0x90; 0x800],
                strings: vec![0, 0],
                functions: BTreeMap::from([(0x1000, 0x1080)]),
            };
            fixture.prologue(0x1000);
            fixture
        }

        fn prologue(&mut self, rva: u32) {
            let offset = (rva - 0x1000) as usize;
            self.text[offset..offset + PROLOGUE.len()].copy_from_slice(&PROLOGUE);
        }

        fn caller(&mut self, root: Option<u32>, call: u32, url: Option<&str>) {
            if let Some(root) = root {
                self.functions.insert(root, root + 0x100);
            }
            let offset = (call - 0x1000) as usize;
            self.text[offset] = 0xE8;
            let displacement = (0x1000_i64 - i64::from(call + 5)) as i32;
            self.text[offset + 1..offset + 5].copy_from_slice(&displacement.to_le_bytes());
            let lea = offset + 0x49;
            if lea + 7 > self.text.len() {
                return;
            }
            let target = match url {
                Some(url) => {
                    let target = 0x4000 + self.strings.len() as u32;
                    self.strings
                        .extend(url.encode_utf16().chain([0]).flat_map(u16::to_le_bytes));
                    target
                }
                None => 0x9000, // Unmapped string address.
            };
            self.text[lea..lea + 3].copy_from_slice(&[0x48, 0x8D, 0x0D]);
            let displacement = (i64::from(target) - i64::from(call + 0x50)) as i32;
            self.text[lea + 3..lea + 7].copy_from_slice(&displacement.to_le_bytes());
        }

        fn image(self, base: u64) -> Image<'static> {
            let pdata: Vec<u8> = self
                .functions
                .into_iter()
                .flat_map(|(start, end)| [start, end, 0x3000])
                .flat_map(u32::to_le_bytes)
                .collect();
            let pdata_end = base + 0x2000 + pdata.len() as u64;
            Image::from_memory(
                base,
                Memory::from_sections(vec![
                    NamedMemorySection::new(
                        ".text".into(),
                        base + 0x1000,
                        SectionKind::Text,
                        self.text,
                    ),
                    NamedMemorySection::new(
                        ".pdata".into(),
                        base + 0x2000,
                        SectionKind::ReadOnlyData,
                        pdata,
                    ),
                    NamedMemorySection::new(
                        ".xdata".into(),
                        base + 0x3000,
                        SectionKind::ReadOnlyData,
                        vec![1, 0, 0, 0],
                    ),
                    NamedMemorySection::new(
                        ".rdata".into(),
                        base + 0x4000,
                        SectionKind::ReadOnlyData,
                        self.strings,
                    ),
                ]),
                ImageType::PEImage(PEImage {
                    exception_directory_range: base + 0x2000..pdata_end,
                    exception_children_cache: Default::default(),
                }),
            )
        }
    }

    fn resolve(image: &Image<'_>) -> patternsleuth::resolvers::Result<RequestFunctions> {
        patternsleuth::resolvers::eval(image, |ctx| {
            Box::pin(async {
                Ok(OpenAPIClientAPI::request_functions()?
                    .resolve(ctx)
                    .await?
                    .value
                    .as_ref()
                    .clone())
            })
        })
    }

    #[test_case::test_case(BASE; "normal image base")]
    #[test_case::test_case(BASE + 0x1000_0000; "rebased image")]
    fn endpoints_keep_their_own_caller_function(base: u64) {
        let mut fixture = Fixture::new();
        fixture.caller(Some(0x1100), 0x1110, Some("https://tbio.invalid/GetMotd"));
        fixture.caller(
            Some(0x1200),
            0x1210,
            Some("https://tbio.invalid/GetCurrentGames"),
        );
        let (address, offsets) = resolve(&fixture.image(base)).unwrap();
        assert_eq!(address, base + 0x1000);
        assert_eq!(
            offsets,
            HashMap::from([
                ("GetPost_GetMotd".into(), 0x1100),
                ("GetPost_GetCurrentGames".into(), 0x1200),
            ])
        );
    }

    #[test]
    fn multiple_calls_in_one_function_do_not_shift_later_mappings() {
        let mut fixture = Fixture::new();
        fixture.caller(Some(0x1100), 0x1110, Some("/GetMotd"));
        fixture.caller(Some(0x1100), 0x1170, Some("/LoginPlayFab"));
        fixture.caller(Some(0x1200), 0x1210, Some("/GetCurrentGames"));
        let (_, offsets) = resolve(&fixture.image(BASE)).unwrap();
        assert_eq!(offsets["GetPost_GetMotd"], 0x1100);
        assert_eq!(offsets["GetPost_LoginPlayFab"], 0x1100);
        assert_eq!(offsets["GetPost_GetCurrentGames"], 0x1200);
    }

    #[test]
    fn unreadable_urls_and_unknown_functions_are_skipped_without_shifting_names() {
        let mut fixture = Fixture::new();
        fixture.caller(Some(0x1100), 0x1110, None);
        fixture.caller(None, 0x1310, Some("/UnknownRoot"));
        fixture.caller(Some(0x1400), 0x1410, Some("/GetCurrentGames"));
        fixture.caller(Some(0x1700), 0x17F0, None); // Displacement outside mapped text.
        let (_, offsets) = resolve(&fixture.image(BASE)).unwrap();
        assert_eq!(
            offsets,
            HashMap::from([("GetPost_GetCurrentGames".into(), 0x1400)])
        );
    }

    #[test]
    fn no_callers_still_resolves_the_request_factory() {
        let (address, offsets) = resolve(&Fixture::new().image(BASE)).unwrap();
        assert_eq!(address, BASE + 0x1000);
        assert!(offsets.is_empty());
    }

    #[test]
    fn multiple_factory_matches_preserve_first_match_selection() {
        let mut fixture = Fixture::new();
        fixture.prologue(0x1600);
        let (address, _) = resolve(&fixture.image(BASE)).unwrap();
        assert_eq!(address, BASE + 0x1000);
    }

    #[test]
    fn missing_factory_pattern_fails() {
        let mut fixture = Fixture::new();
        fixture.text.fill(0x90);
        assert!(resolve(&fixture.image(BASE)).is_err());
    }

    #[test]
    fn registered_custom_resolver_publishes_offsets_and_uses_the_resolver_cache() {
        let mut fixture = Fixture::new();
        fixture.caller(Some(0x1100), 0x1110, Some("/GetMotd"));
        let image = fixture.image(BASE);
        STRING_TO_ROOT_FN
            .write()
            .unwrap()
            .insert("GetPost_Stale".into(), 0xDEAD);
        patternsleuth::resolvers::eval(&image, |ctx| {
            Box::pin(async {
                let first = ctx.resolve(CreateHttpRequest::resolver()).await.unwrap();
                let cached = ctx.resolve(CreateHttpRequest::resolver()).await.unwrap();
                assert!(std::sync::Arc::ptr_eq(&first, &cached));
                assert_eq!(first.0, BASE + 0x1000);
            })
        });
        let registry = inventory::iter::<crate::resolvers::OffsetRegisty>
            .into_iter()
            .find(|registry| registry.name == "CreateHttpRequest Get*Post hooks")
            .unwrap();
        assert_eq!(
            (registry.map)(),
            HashMap::from([("GetPost_GetMotd".into(), 0x1100)])
        );

        let mut missing = Fixture::new();
        missing.text.fill(0x90);
        let error = missing
            .image(BASE)
            .resolve(CreateHttpRequest::resolver())
            .unwrap_err();
        assert!(error.to_string().contains("CreateHttpRequest"), "{error}");
        assert_eq!(
            (registry.map)(),
            HashMap::from([("GetPost_GetMotd".into(), 0x1100)])
        );
    }
}

// OpenAPIMerged::OpenAPIClientApi::GetCurrentGamesPostResponse.
pub type GetCurrentGamesPostResponse = c_void;
