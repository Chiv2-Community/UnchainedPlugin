use std::ffi::c_void;

// Native types used only behind pointers; these aliases do not describe layouts
// or provide distinct type identities or thread-safety guarantees.
pub type FConfigCacheIni = c_void;
pub type UEngine = c_void;
pub type FWorldContext = c_void;
pub type UPendingNetGame = c_void;
pub type UTBLGameInstance = c_void;
pub type AGameModeBase = c_void;
pub type AGameMode = c_void;
pub type UGameEngine = c_void;
pub type APlayerController = c_void;
pub type FGuid = c_void;
pub type FTBLOnlineStats = c_void;
pub type FPrimaryAssetId = c_void;
pub type UAssetManager = c_void;
pub type IAssetRegistry = c_void;
pub type UAssetRegistryImpl = c_void;
pub type UPackageMap = c_void;
pub type FUObjectSerializeContext = c_void;
pub type USaveGame = c_void;
pub type FPakPlatformFile = c_void;
pub type FPakFile = c_void;
pub type FPakEntry = c_void;
// FPakPlatformFile::FPakListEntry.
pub type FPakListEntry = c_void;
pub type FConsoleOutputDevice = c_void;
pub type FOnlineSessionPlayFab = c_void;
// OpenAPIMerged::OpenAPIClientApi::GetCurrentGamesPostResponse.
pub type GetCurrentGamesPostResponse = c_void;
// Template specializations, not pointers to the wrapped UClass/IHttpRequest.
pub type TSubclassOfULoadoutSelection = c_void; // TSubclassOf<ULoadoutSelection>
pub type TSubclassOfAInventoryItem = c_void; // TSubclassOf<AInventoryItem>
pub type TSubclassOfATBLCharacter = c_void; // TSubclassOf<ATBLCharacter>
pub type TSharedRefIHttpRequest = c_void; // TSharedRef<IHttpRequest, 0>
pub type TSharedPtrIHttpRequest = c_void; // TSharedPtr<IHttpRequest, 0>
pub type TFunctionFIcmpEchoResult = c_void; // TFunction<void(FIcmpEchoResult)>
