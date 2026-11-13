use std::ffi::c_void;

// Native types used only behind pointers; these aliases do not describe layouts
// or provide distinct type identities or thread-safety guarantees.
pub type UTBLGameInstance = c_void;
pub type AGameMode = c_void;
pub type UGameEngine = c_void;
pub type APlayerController = c_void;
pub type FGuid = c_void;
pub type FTBLOnlineStats = c_void;
pub type FPrimaryAssetId = c_void;
pub type UAssetManager = c_void;
pub type IAssetRegistry = c_void;
pub type UAssetRegistryImpl = c_void;
/// Opaque UE archive base. Its layout/vtable is target-build-specific; Rust
/// must not construct or dereference it until the reader constructor boundary
/// is resolved and verified.
#[repr(C)]
pub struct FArchive {
    _opaque: [u8; 0],
}
/// Opaque `FLargeMemoryReader`/`FMemoryReader` implementation. This is kept
/// distinct from `FArchive` so future verified construction code cannot pass a
/// base pointer where an owning reader is required.
#[repr(C)]
pub struct FLargeMemoryReader {
    _opaque: [u8; 0],
}
/// Opaque, engine-owned Asset Registry state. It contains UE containers and
/// must be constructed/destructed by verified UE calls, never Rust allocation.
#[repr(C)]
pub struct FAssetRegistryState {
    _opaque: [u8; 0],
}
/// Opaque serialization options owned by `UAssetRegistryImpl` on the target
/// build. No Rust layout is assumed.
#[repr(C)]
pub struct FAssetRegistrySerializationOptions {
    _opaque: [u8; 0],
}
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
