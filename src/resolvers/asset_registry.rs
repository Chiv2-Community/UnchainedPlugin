use crate::ue::{
    EFindName, FName, FString, FUObjectSerializeContext, IAssetRegistry, TArray, UAssetManager,
    TScriptInterface, UClass, UObject, UPackageMap, TCHAR,
};
use widestring::U16CStr;

// Signature source: owner-provided target-build bytes for
// `FLargeMemoryReader::FLargeMemoryReader`. The constructor is used with
// `ELargeMemoryReaderFlags::None` so it borrows Rust-owned bytes during the
// synchronous registry load. Its rel32 call and RIP-relative vtable address
// are wildcarded.
define_pattern_resolver!(FLargeMemoryReaderCtor, [
    "48 89 5C 24 08 48 89 6C 24 10 48 89 74 24 18 57 48 83 EC 30 41 0F B6 F1 49 8B E8 48 8B DA 48 8B F9 E8 ?? ?? ?? ?? 80 A7 98 00 00 00 FE 48 8D 15 ?? ?? ?? ?? 40 0F B6 C6 48 89 17 24 01 48 89 9F A0 00 00 00"
]);
// FLargeMemoryReader* FLargeMemoryReader::FLargeMemoryReader(
//     FLargeMemoryReader*, const uint8*, int64, ELargeMemoryReaderFlags, FName)
CREATE_HOOK!(FLargeMemoryReaderCtor, CALLED, *mut crate::ue::FLargeMemoryReader, (
    this_ptr: *mut crate::ue::FLargeMemoryReader,
    data: *const u8,
    size: i64,
    flags: u8,
    archive_name: FName
), {
    CALL_ORIGINAL!(FLargeMemoryReaderCtor(this_ptr, data, size, flags, archive_name))
});

// Signature source: owner-provided target-build bytes for
// `FLargeMemoryReader::~FLargeMemoryReader`. Wildcard the RIP-relative vtable
// address, the `FMemory::Free` rel32 call, and the tail jump to base cleanup.
define_pattern_resolver!(FLargeMemoryReaderDtor, [
    "40 53 48 83 EC 20 F6 81 98 00 00 00 01 48 8D 05 ?? ?? ?? ?? 48 89 01 48 8B D9 74 0C 48 8B 89 A0 00 00 00 E8 ?? ?? ?? ?? 48 8B CB 48 83 C4 20 5B E9 ?? ?? ?? ??"
]);
// void FLargeMemoryReader::~FLargeMemoryReader(FLargeMemoryReader* this)
CREATE_HOOK!(FLargeMemoryReaderDtor, CALLED, NONE, (), (
    this_ptr: *mut crate::ue::FLargeMemoryReader
), {
    CALL_ORIGINAL!(FLargeMemoryReaderDtor(this_ptr))
});

define_pattern_resolver!(GetAssetRegistry,[
    "40 57 48 83 EC 20 48 8B 81 ?? ?? ?? ?? 48 8B F9 48 85 C0 75 ?? 44 8D 40 ??"
]);
// IAssetRegistry * __thiscall UAssetManager::GetAssetRegistry(UAssetManager *this)
CREATE_HOOK!(GetAssetRegistry, CALLED, *mut IAssetRegistry, (asset_manager: *mut UAssetManager),{
    // sinfo!["Triggered!"];
});

define_pattern_resolver!(UAssetRegistryHelpers__GetAssetRegistry,[
    "48 89 5C 24 ?? 57 48 83 EC 20 48 8B F9 48 8D 15 ?? ?? ?? ?? 48 8D 4C 24 ?? 41 B8 01 00 00 00 E8 ?? ?? ?? ?? 48 8B 18 E8 ?? ?? ?? ?? 48 8B D3 48 8B C8 E8 ?? ?? ?? ?? 48 8B C8 48 8B 10 FF 52 ?? 48 85 C0"
    ]);
// TScriptInterface<> * __cdecl UAssetRegistryHelpers::GetAssetRegistry(TScriptInterface<> *__return_storage_ptr__)
CREATE_HOOK!(UAssetRegistryHelpers__GetAssetRegistry, CALLED,  *mut TScriptInterface, (ret_val_tscr:  *mut TScriptInterface),{
    sdebug!["Triggered!"];
});

define_pattern_resolver!(FNameCtorWchar, Simple, ["48 89 5C 24 ?? 57 48 83 EC 30 48 8B D9 48 89 54 24 ??"]);
// FName * __thiscall FName::FName(FName *this,wchar_t *param_1,EFindName param_2)
CREATE_HOOK!(FNameCtorWchar, CALLED,  *mut FName, (this: *mut FName, Str: *const TCHAR, findname: EFindName),{
    // sinfo!["Triggered!"];
    unsafe {
        if findname == EFindName::Find {
            let fstring = FString::from(
                widestring::U16CString::from_ptr_str(Str)
                .as_slice_with_nul());
            if fstring.to_string().contains("ModMarker") {
                serror!("Name: {}", fstring);
            }
        }
    }
});

define_pattern_resolver!(StaticFindObject,["48 89 5C 24 08 48 89 74 24 18 55 57 41 54 41 56 41 57 48 8B EC 48 83 EC 60 80 3D 98 BB"]);
// UObject* StaticFindObject( UClass* ObjectClass, UObject* InObjectPackage, const TCHAR* OrigInName, bool ExactClass )
CREATE_HOOK!(StaticFindObject, CALLED, *mut UObject, (
    obj_class: *mut UClass,
    package: *mut UObject,
    name: *const TCHAR,
    exact_class: bool
), {
    if !name.is_null() {
        let name_str = unsafe { widestring::U16CStr::from_ptr_str(name) };
        sinfo!("Looking for object: {}", name_str.to_string_lossy());
    }
});

define_pattern_resolver!(GetDefaultObject,["40 53 48 83 EC 20 48 8B 81 18 01 00 00 48 8B D9 48 85 C0 ?? ?? 84 D2 ?? ?? 48 8B 01 ?? ?? ?? ?? ?? ??"]);
// UObject* GetDefaultObject(bool bCreateIfNeeded = true) const
CREATE_HOOK!(GetDefaultObject, CALLED, *mut UObject, (this_ptr: *mut UClass, create_if_needed: bool),{
    sdebug!["Triggered!"];
});

define_pattern_resolver!(GetAllActorsOfClass,["40 55 53 56 57 48 8D 6C 24 C1 48 81 EC A8 00 00 00 48 8B 05 70 B1 70 02 48 33 C4 48 89 45 1F 41"]);
// static void GetAllActorsOfClass(const UObject* WorldContextObject, TSubclassOf<AActor> ActorClass, TArray<AActor*>& OutActors);
CREATE_HOOK!(GetAllActorsOfClass, CALLED, (WorldContextObject: *const UObject, ActorClass: *mut UClass, OutActors: *mut TArray<*mut UObject>),{
    sdebug!["Triggered!"];
});

define_pattern_resolver!(StaticLoadObject,["40 55 53 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 28 FA FF FF 48 81 EC D8 06 00 00 48 8B 05 25 5E 9F 03 48 33 C4 48 89 85 C0 05 00 00 48"]);
// UObject* StaticLoadObject(UClass* ObjectClass, UObject* InOuter, const TCHAR* InName, const TCHAR* Filename, uint32 LoadFlags, UPackageMap* Sandbox, bool bAllowObjectReconciliation, FUObjectSerializeContext* InSerializeContext)
// UObject* StaticLoadObject(UClass* Class, UObject* InOuter, const TCHAR* Name, const TCHAR* Filename, uint32 LoadFlags, UPackageMap* Sandbox, bool bAllowNativeComponentClass, const FLinkerInstancingContext* InstancingContext)
// The resolved target uses FUObjectSerializeContext, not FLinkerInstancingContext.
CREATE_HOOK!(StaticLoadObject, CALLED, *mut UObject, (
    obj_class: *mut UClass,
    in_outer: *mut UObject,
    name: *const TCHAR,
    filename: *const TCHAR,
    load_flags: u32,
    sandbox: *mut UPackageMap,
    allow_native_comp: bool,
    instancing_context: *mut FUObjectSerializeContext
), {
    if !name.is_null() {
        let name_str = unsafe { U16CStr::from_ptr_str(name) };
        swarn!("Loading object: {}", name_str.to_string_lossy());
    }
});



// TODO: Conv_InterfaceToObject unused
// let rel_address = *globals().resolution.kismet_system_library.0.get("Conv_InterfaceToObject").unwrap() as usize;
// void __cdecl UKismetSystemLibrary::execConv_InterfaceToObject(UObject *param_1,FFrame *param_2,void *param_3)
// #[cfg(feature = "dev")]
// CREATE_HOOK!(Conv_InterfaceToObject, INACTIVE, (object: *mut c_void, frame: *mut c_void, arg3: *mut c_void),{
//     sinfo!["Triggered!"];
// });


// /// FNamePool
// #[derive(Debug, PartialEq)]
// #[cfg_attr(
//     feature = "serde-resolvers",
//     derive(serde::Serialize, serde::Deserialize)
// )]
// pub struct FNamePool(pub usize);
// impl_resolver_singleton!(all, FNamePool, |ctx| async {
//     let patterns = [
//         "74 09 4C 8D 05 | ?? ?? ?? ?? EB ?? 48 8D 0D ?? ?? ?? ?? E8",
//         "48 83 EC 20 C1 EA 03 48 8d 2d | ?? ?? ?? ?? ?? CA ?? ?? 48 bf cd cc cc cc cc cc cc",
//     ];

//     let res = join_all(patterns.iter().map(|p| ctx.scan(Pattern::new(p).unwrap()))).await;

//     Ok(Self(try_ensure_one(res.iter().flatten().map(
//         |a| -> Result<usize> { Ok(ctx.image().memory.rip4(*a)?) },
//     ))?))
// });


// hmm
// #[cfg(feature = "dev")]
// define_pattern_resolver!(FNamePool, Call, [
//         "74 09 4C 8D 05 | ?? ?? ?? ?? EB ?? 48 8D 0D ?? ?? ?? ?? E8",
//         "48 83 EC 20 C1 EA 03 48 8d 2d | ?? ?? ?? ?? ?? CA ?? ?? 48 bf cd cc cc cc cc cc cc",
//     ]);

// #[cfg(feature = "dev")]
// CREATE_HOOK!(FNamePool, INACTIVE,  *mut c_void, (ret_val_tscr:  *mut c_void),{
//     sinfo!["Triggered!"];
// });

// TODO: GetAsset  Unused
// define_pattern_resolver!(GetAsset,["40 53 48 83 EC 60 48 8B D9 33 D2"]); // 2.11.4
// CREATE_HOOK!(GetAsset, INACTIVE, *mut UObject, (asset_data: *mut FAssetData),{
//     // unsafe {
//     //     // sinfo!["Triggered! {}", &*asset_data];
//     // }
// });

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_asset_registry_preserves_pointer_return_signature() {
        let _: unsafe extern "C" fn(*mut UAssetManager) -> *mut IAssetRegistry =
            GetAssetRegistry_detour_fkt;
        let _: fn(*mut UAssetManager) -> *mut IAssetRegistry =
            |asset_manager| CALL_ORIGINAL!(GetAssetRegistry(asset_manager));
    }
}
