use crate::ue::{
    EFindName, FName, FString, FUObjectSerializeContext, IAssetRegistry, TArray, UAssetManager,
    UAssetRegistryImpl, UClass, UObject, UPackageMap, TCHAR,
};
use std::{fmt, os::raw::c_void};
use widestring::U16CStr;

/// Target-build call boundary, intentionally not backed by a resolver yet.
///
/// Populate only after the owner supplies a current-build byte string or a
/// confirmed vtable slot. See `.opencode/skills/ue-signature-wildcards` before
/// adding a pattern: relocation-sensitive bytes must be wildcarded, but a
/// pattern must never be invented from an ABI declaration.
pub type FnFAssetRegistryStateSerialize = unsafe extern "system" fn(
    state: *mut crate::ue::FAssetRegistryState,
    archive: *mut crate::ue::FArchive,
    options: *const crate::ue::FAssetRegistrySerializationOptions,
) -> bool;

/// Target-build call boundary for the additive native merge. Deliberately
/// unresolved until a verified signature or `IAssetRegistry` vtable slot is
/// available; no placeholder address is callable.
pub type FnUAssetRegistryImplAppendState = unsafe extern "system" fn(
    registry: *mut UAssetRegistryImpl,
    state: *const crate::ue::FAssetRegistryState,
);

// Signature source: owner-provided target-build bytes for
// `FAssetRegistryState::Serialize`. The RIP-relative displacement in the
// `48 8B 05` stack-cookie load is wildcarded: it relocates on each rebuild,
// while the surrounding `48 33 C4` confirms the instruction's purpose.
define_pattern_resolver!(FAssetRegistryStateSerialize, [
    "48 89 5C 24 18 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 00 FD FF FF 48 81 EC 00 04 00 00 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 85 F0 02 00 00 F6 42 28 02 4C 8B E9 48 89 4D B0"
]);
// bool FAssetRegistryState::Serialize(FArchive&, const FAssetRegistrySerializationOptions&)
CREATE_HOOK!(FAssetRegistryStateSerialize, CALLED, NONE, bool, (
    state: *mut crate::ue::FAssetRegistryState,
    archive: *mut crate::ue::FArchive,
    options: *const crate::ue::FAssetRegistrySerializationOptions
), {
    // CALLED hooks are not activated on attach. The merge path remains gated
    // until the archive reader and AppendState target are both verified.
    CALL_ORIGINAL!(FAssetRegistryStateSerialize(state, archive, options))
});

define_pattern_resolver!(GetAssetRegistry,[
    "40 57 48 83 EC 20 48 8B 81 ?? ?? ?? ?? 48 8B F9 48 85 C0 75 ?? 44 8D 40 ??"
]);
// IAssetRegistry * __thiscall UAssetManager::GetAssetRegistry(UAssetManager *this)
CREATE_HOOK!(GetAssetRegistry, CALLED, *mut IAssetRegistry, (asset_manager: *mut UAssetManager),{
    // sinfo![fn; "Triggered!"];
});

#[cfg(test)]
mod hook_signature_tests {
    use super::*;

    #[test]
    fn get_asset_registry_preserves_pointer_return_signature() {
        // Both the detour entry point and original trampoline must return the
        // native IAssetRegistry pointer, even when the hook body ignores it.
        let _: unsafe extern "C" fn(*mut UAssetManager) -> *mut IAssetRegistry =
            GetAssetRegistry_detour_fkt;
        let _: fn(*mut UAssetManager) -> *mut IAssetRegistry =
            |asset_manager| CALL_ORIGINAL!(GetAssetRegistry(asset_manager));
    }
}


#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(non_snake_case)]
pub struct FAssetData {
    pub ObjectPath: FName,
    pub PackageName: FName,
    pub PackagePath: FName,
    pub AssetName: FName,
    pub AssetClass: FName,
    pub TagsAndValues: [u8; 0x10], // TSharedPtr<FAssetDataTagMap,0>
    pub ChunkIDs: [u8; 0x10], // TArray<int> ?
    pub PackageFlags: u32,
    pub padding: [u8; 4], // for alignment to 0x50
}

impl fmt::Display for FAssetData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ObjectPath {}\n PackageName: {}\n PackagePath: {}\n AssetName: {}\n AssetClass: {}", 
        self.ObjectPath,
        self.PackageName,
        self.PackagePath,
        self.AssetName,
        self.AssetClass)
        // write!(f, "ObjectPath {}", 
        // self.ObjectPath)
    }
}

define_pattern_resolver!(GetAssetsByClass,[
    "48 89 5C 24 ?? 55 56 57 41 56 41 57 48 8D 6C 24 ?? 48 81 EC 20 01 00 00 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 45 ?? 45 33 FF C7 45 ?? 80 00 00 00 0F 57 C0 4C 89 7C 24 ?? 48 8B DA 4C 89 7C 24 ??"
    ]);

// bool __thiscall UAssetRegistryImpl::GetAssetsByClass(UAssetRegistryImpl *this,FName param_1,TArray<> *param_2,bool param_3)
CREATE_HOOK!(GetAssetsByClass, CALLED, NONE, bool, (this_ptr: *mut UAssetRegistryImpl, ClassPathName: FName, OutAssetData: *mut TArray<FAssetData>, bSearchSubClasses: bool), {
    // Debug code
    sinfo![fn; "Triggered! {}", ClassPathName];
    let res: bool;
    unsafe {
        res = CALL_ORIGINAL!(GetAssetsByClass(this_ptr, ClassPathName, OutAssetData, bSearchSubClasses));
        if res && ClassPathName.to_string() == "DA_ModMarker_C" {
            swarn!(fn; "this_ptr: {:#?}", this_ptr);
            swarn!(fn; "ClassPathName: {}", ClassPathName);
            // sinfo!(fn; "ClassPathName D: {:#?}", ClassPathName);
            swarn!(fn; "OutAssetData: {:#?}", &*OutAssetData);
            swarn!(fn; "bSearchSubClasses: {}", bSearchSubClasses);
            // let asdf = &*OutAssetData;
            let asdf = &*OutAssetData;
            let test_cp = asdf.clone();
            sinfo![fn; "Length res: {}", test_cp.len()];

            for (cnt, a) in test_cp.as_slice().iter().enumerate() {
                sinfo!(fn; "{}: {}", cnt, a.PackagePath);
            }
        }
    }
    res
});

// Signature source: owner-provided pattern for
// `UAssetRegistryImpl::GetAssetsByPath` (function start). The RIP-relative
// displacement in the `48 8B 05` stack-cookie load (immediately followed by
// the `48 33 C4` xor with rsp) is wildcarded so the pattern survives
// rebuilds that relocate the cookie global.
define_pattern_resolver!(GetAssetsByPath,[
    "40 55 53 56 57 41 56 48 8D 6C 24 E0 48 81 EC 20 01 00 00 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 45 10 45"
]);
// bool __thiscall UAssetRegistryImpl::GetAssetsByPath(UAssetRegistryImpl *this,FName PackagePath,TArray<FAssetData>& OutAssetData,bool bRecursive,bool bIncludeOnlyOnDiskAssets)
// virtual bool GetAssetsByPath(FName PackagePath, TArray<FAssetData>& OutAssetData, bool bRecursive = false, bool bIncludeOnlyOnDiskAssets = false) const
CREATE_HOOK!(GetAssetsByPath, CALLED, NONE, bool, (this_ptr: *mut UAssetRegistryImpl, PackagePath: FName, OutAssetData: *mut TArray<FAssetData>, bRecursive: bool, bIncludeOnlyOnDiskAssets: bool), {
    // CALLED hooks are never enabled on attach; this body only runs if someone
    // explicitly enables the detour after a real signature is populated.
    unsafe {
        CALL_ORIGINAL!(GetAssetsByPath(this_ptr, PackagePath, OutAssetData, bRecursive, bIncludeOnlyOnDiskAssets))
    }
});

// Signature source: owner-provided pattern for
// `UAssetRegistryImpl::ScanPathsAndFilesSynchronous` (function start). The
// RIP-relative displacement in the `48 8B 05` load is wildcarded so the
// pattern survives rebuilds that relocate the pointed-to global.
define_pattern_resolver!(ScanPathsAndFilesSynchronous,[
    "48 89 5C 24 20 55 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 10 FE FF FF 48 81 EC F0 02 00 00 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 85 E0 01 00 00 48 8B 85 58 02"
]);
// void __thiscall UAssetRegistryImpl::ScanPathsAndFilesSynchronous(UAssetRegistryImpl *this,const TArray<FString>& InPaths,const TArray<FString>& InSpecificFiles,bool bForceRescan,EAssetDataCacheMode AssetDataCacheMode,TArray<FName>* OutFoundAssets,TArray<FName>* OutFoundPaths)
// Implementation both public wrappers (ScanPathsSynchronous / ScanFilesSynchronous) delegate to.
CREATE_HOOK!(ScanPathsAndFilesSynchronous, CALLED, NONE, (), (this_ptr: *mut UAssetRegistryImpl, InPaths: *const TArray<FString>, InSpecificFiles: *const TArray<FString>, bForceRescan: bool, AssetDataCacheMode: u8, OutFoundAssets: *mut TArray<FName>, OutFoundPaths: *mut TArray<FName>), {
    // CALLED hooks are never enabled on attach; this body only runs if someone
    // explicitly enables the detour after a real signature is populated.
    unsafe {
        CALL_ORIGINAL!(ScanPathsAndFilesSynchronous(this_ptr, InPaths, InSpecificFiles, bForceRescan, AssetDataCacheMode, OutFoundAssets, OutFoundPaths))
    }
});

// Signature source: owner-provided pattern for the private
// `UAssetRegistryImpl::AddAssetData` — the fallback registration path.
define_pattern_resolver!(AddAssetData,[
    "40 55 53 56 57 48 8B EC 48 83 EC 78 48 8B F9 48 8B F2 48 83 C1 30 ?? ?? ?? ?? ?? 48 8D 8F 90"
]);
// void UAssetRegistryImpl::AddAssetData(FAssetData* AssetData)  // private
CREATE_HOOK!(AddAssetData, CALLED, NONE, (), (this_ptr: *mut UAssetRegistryImpl, AssetData: *mut FAssetData), {
    unsafe {
        CALL_ORIGINAL!(AddAssetData(this_ptr, AssetData))
    }
});

#[repr(C)]
#[derive(Debug)]
pub struct TScriptInterface {
    // padding: [u8; 0x28], // can be found in cxxheaderfiles
    pub object: *mut UObject,
    pub interface: *mut c_void,
}

impl TScriptInterface {
    pub fn new() -> Self {
        Self {
            // padding: [0u8; 0x28],
            object: std::ptr::null_mut(),
            interface: std::ptr::null_mut(),
        }
    }
}

define_pattern_resolver!(GetAssetRegistry_Helper,[
    "48 89 5C 24 ?? 57 48 83 EC 20 48 8B F9 48 8D 15 ?? ?? ?? ?? 48 8D 4C 24 ?? 41 B8 01 00 00 00 E8 ?? ?? ?? ?? 48 8B 18 E8 ?? ?? ?? ?? 48 8B D3 48 8B C8 E8 ?? ?? ?? ?? 48 8B C8 48 8B 10 FF 52 ?? 48 85 C0"
    ]);
// TScriptInterface<> * __cdecl UAssetRegistryHelpers::GetAssetRegistry(TScriptInterface<> *__return_storage_ptr__)
CREATE_HOOK!(GetAssetRegistry_Helper, CALLED,  *mut TScriptInterface, (ret_val_tscr:  *mut TScriptInterface),{
    sinfo![fn; "Triggered!"];
});

define_pattern_resolver!(FNameCtorWchar, Simple, ["48 89 5C 24 ?? 57 48 83 EC 30 48 8B D9 48 89 54 24 ??"]);
// FName * __thiscall FName::FName(FName *this,wchar_t *param_1,EFindName param_2)
CREATE_HOOK!(FNameCtorWchar, CALLED,  *mut FName, (this: *mut FName, Str: *const TCHAR, findname: EFindName),{
    // sinfo![fn; "Triggered!"];
    unsafe {
        if findname == EFindName::Find {
            let fstring = FString::from(
                widestring::U16CString::from_ptr_str(Str)
                .as_slice_with_nul());
            if fstring.to_string().contains("ModMarker") {
                serror!(fn; "Name: {}", fstring);
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
        sinfo!(fn; "Looking for object: {}", name_str.to_string_lossy());
    }
});

define_pattern_resolver!(GetDefaultObject,["40 53 48 83 EC 20 48 8B 81 18 01 00 00 48 8B D9 48 85 C0 ?? ?? 84 D2 ?? ?? 48 8B 01 ?? ?? ?? ?? ?? ??"]);
// UObject* GetDefaultObject(bool bCreateIfNeeded = true) const
CREATE_HOOK!(GetDefaultObject, CALLED, *mut UObject, (this_ptr: *mut UClass, create_if_needed: bool),{
    sinfo![fn; "Triggered!"];
});

define_pattern_resolver!(GetAllActorsOfClass,["40 55 53 56 57 48 8D 6C 24 C1 48 81 EC A8 00 00 00 48 8B 05 70 B1 70 02 48 33 C4 48 89 45 1F 41"]);
// static void GetAllActorsOfClass(const UObject* WorldContextObject, TSubclassOf<AActor> ActorClass, TArray<AActor*>& OutActors);
CREATE_HOOK!(GetAllActorsOfClass, CALLED, (WorldContextObject: *const UObject, ActorClass: *mut UClass, OutActors: *mut TArray<*mut UObject>),{
    sinfo![fn; "Triggered!"];
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
        swarn!(fn; "Loading object: {}", name_str.to_string_lossy());
    }
});



// TODO: Conv_InterfaceToObject unused
// let rel_address = *globals().resolution.kismet_system_library.0.get("Conv_InterfaceToObject").unwrap() as usize;
// void __cdecl UKismetSystemLibrary::execConv_InterfaceToObject(UObject *param_1,FFrame *param_2,void *param_3)
// #[cfg(feature = "dev")]
// CREATE_HOOK!(Conv_InterfaceToObject, INACTIVE, (object: *mut c_void, frame: *mut c_void, arg3: *mut c_void),{
//     sinfo![fn; "Triggered!"];
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
//     sinfo![fn; "Triggered!"];
// });

// TODO: GetAsset  Unused
// define_pattern_resolver!(GetAsset,["40 53 48 83 EC 60 48 8B D9 33 D2"]); // 2.11.4
// CREATE_HOOK!(GetAsset, INACTIVE, *mut UObject, (asset_data: *mut FAssetData),{
//     // unsafe {
//     //     // sinfo![fn; "Triggered! {}", &*asset_data];
//     // }
// });
