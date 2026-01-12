


// IAssetRegistry * __thiscall UAssetManager::GetAssetRegistry(UAssetManager *this)

use std::{fmt, os::raw::c_void};

use widestring::U16CStr;

use crate::{ serror, sinfo, swarn, ue::{EFindName, FName, FString, TArray, UClass, UObject}};

#[cfg(feature = "dev")]
define_pattern_resolver!(GetAssetRegistry,[
    "40 57 48 83 EC 20 48 8B 81 ?? ?? ?? ?? 48 8B F9 48 85 C0 75 ?? 44 8D 40 ??"
]);

// TODO: We only need to call this
#[cfg(feature = "dev")]
CREATE_HOOK!(GetAssetRegistry, INACTIVE, (asset_manager: *mut c_void),{
    // crate::sinfo![f; "Triggered!"];
});


// let rel_address = *globals().resolution.kismet_system_library.0.get("Conv_InterfaceToObject").unwrap() as usize;
// void __cdecl UKismetSystemLibrary::execConv_InterfaceToObject(UObject *param_1,FFrame *param_2,void *param_3)
#[cfg(feature = "dev")]
CREATE_HOOK!(Conv_InterfaceToObject, INACTIVE, (object: *mut c_void, frame: *mut c_void, arg3: *mut c_void),{
    crate::sinfo![f; "Triggered!"];
});

// bool __thiscall UAssetRegistryImpl::GetAssetsByClass(UAssetRegistryImpl *this,FName param_1,TArray<> *param_2,bool param_3)


#[cfg(feature = "dev")]
define_pattern_resolver!(GetAssetsByClass,[
    "48 89 5C 24 ?? 55 56 57 41 56 41 57 48 8D 6C 24 ?? 48 81 EC 20 01 00 00 48 8B 05 ?? ?? ?? ?? 48 33 C4 48 89 45 ?? 45 33 FF C7 45 ?? 80 00 00 00 0F 57 C0 4C 89 7C 24 ?? 48 8B DA 4C 89 7C 24 ??"
    ]);


    // FName ObjectPath
    // FName PackageName
    // FName PackagePath
    // FName AssetName
    // FName AssetClass
    // TSharedPtr<FAssetDataTagMap,0> TagsAndValues
    // TArray<int,TSizedDefaultAllocator<32>_> {
    //    0   ForAnyElementType   8   AllocatorInstance   ""
    //    8   int   4   ArrayNum   ""
    //    12   int   4   ArrayMax   ""
    // } ChunkIDs
    // uint	PackageFlags

// #[derive(Debug, Clone)]
// pub struct FAssetData {
//     pub ObjectPath: FName,
//     pub PackageName: FName,
//     pub PackagePath: FName,
//     pub AssetName: FName,
//     pub AssetClass: FName,

//     // TagsAndValues: [u8; 0x8],
//     TagsAndValues: *mut u8,
//     // // TagsAndValues2: [u8; 0x8],
//     ChunkIDs: [u8; 0x18],

//     PackageFlags: u32,
// }

#[repr(C)]
#[derive(Debug, Clone, Copy)]
#[allow(non_snake_case)]
pub struct FAssetData {
    pub ObjectPath: FName,
    pub PackageName: FName,
    pub PackagePath: FName,
    pub AssetName: FName,
    pub AssetClass: FName,
    pub TagsAndValues: [u8; 0x10],
    pub ChunkIDs: [u8; 0x10],
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

// #[cfg(feature = "dev")]
// CREATE_HOOK!(GetAssetsByClass, POST, bool, (this_ptr: *mut c_void, ClassPathName: FName, OutAssetData: TArray<FAssetData>, bSearchSubClasses: bool), |ret_val: bool| {
//     crate::sinfo![f; "Triggered! {}", ClassPathName];
//     // let slc = OutAssetData.as_slice();
//     // let slc = OutAssetData.len();
//     // crate::sinfo![f; "Length res: {}", OutAssetData.clone().len()];
//     // for asd in OutAssetData.as_slice() {

//     // }
    
//     ret_val
// });

// static mut test_name: FName = FName;

#[cfg(feature = "dev")]
CREATE_HOOK!(GetAssetsByClass, INACTIVE, NONE, bool, (this_ptr: *mut c_void, ClassPathName: FName, OutAssetData: *mut TArray<FAssetData>, bSearchSubClasses: bool), {
    crate::sinfo![f; "Triggered! {}", ClassPathName];
    // let slc = OutAssetData.as_slice();
    // let slc = OutAssetData.len();
    // let test = OutAssetData.clone();
    let res: bool;
    unsafe {
        res = o_GetAssetsByClass.call(this_ptr, ClassPathName, OutAssetData, bSearchSubClasses);
        // if ClassPathName.to_string() == "DA_ModMarker_C".to_string() {

        // }
        if res && ClassPathName.to_string() == "DA_ModMarker_C" {
            swarn!(f; "this_ptr: {:#?}", this_ptr);
            swarn!(f; "ClassPathName: {}", ClassPathName);
            // sinfo!(f; "ClassPathName D: {:#?}", ClassPathName);
            swarn!(f; "OutAssetData: {:#?}", &*OutAssetData);
            swarn!(f; "bSearchSubClasses: {}", bSearchSubClasses);
            // let asdf = &*OutAssetData;
            let asdf = &*OutAssetData;
            let test_cp = asdf.clone();
            crate::sinfo![f; "Length res: {}", test_cp.len()];
            // let val1 = &test_cp.as_slice()[0];
            // let val2 = &test_cp.as_slice()[1];
            
            // sinfo!(f; "{}: Asset: {}", 1, val1.ObjectPath);
            // sinfo!(f; "{}: Asset: {}", 1, val1.PackageName);
            // sinfo!(f; "{}: Asset: {}", 1, val1.PackagePath);
            // sinfo!(f; "{}: Asset: {}", 1, val1.AssetName);
            // sinfo!(f; "{}: Asset: {}", 1, val1.AssetClass);
            // sinfo!(f; "{}: Asset: {}", 2, val2.ObjectPath);
            // sinfo!(f; "{}: Asset: {}", 2, val2.PackageName);

            for (cnt, a) in test_cp.as_slice().iter().enumerate() {
                // sinfo!(f; "{}: \n {}", cnt, a);
                sinfo!(f; "{}: {}", cnt, a.PackagePath);
                // sinfo!(f; "Package {}: {}", a.AssetName, a.PackagePath)
            }
        }
    }
    // crate::sinfo![f; "Length res: {}", OutAssetData.clone().len()];
    // for asd in OutAssetData.as_slice() {

    // }
    
    // ret_val
    res
});

// TScriptInterface<> * __cdecl UAssetRegistryHelpers::GetAssetRegistry(TScriptInterface<> *__return_storage_ptr__)

#[repr(C)]
#[derive(Debug)]
pub struct TScriptInterface {
    // padding: [u8; 0x28], // can be found in cxxheaderfiles
    pub object: *mut UObject,
    pub interface: *mut c_void,
}
#[allow(dead_code)]
impl TScriptInterface {
    pub fn new() -> Self {
        Self {
            // padding: [0u8; 0x28],
            object: std::ptr::null_mut(),
            interface: std::ptr::null_mut(),
        }
    }
}

#[cfg(feature = "dev")]
define_pattern_resolver!(GetAssetRegistry_Helper,[
    "48 89 5C 24 ?? 57 48 83 EC 20 48 8B F9 48 8D 15 ?? ?? ?? ?? 48 8D 4C 24 ?? 41 B8 01 00 00 00 E8 ?? ?? ?? ?? 48 8B 18 E8 ?? ?? ?? ?? 48 8B D3 48 8B C8 E8 ?? ?? ?? ?? 48 8B C8 48 8B 10 FF 52 ?? 48 85 C0"
    ]);

#[cfg(feature = "dev")]
CREATE_HOOK!(GetAssetRegistry_Helper, INACTIVE,  *mut TScriptInterface, (ret_val_tscr:  *mut TScriptInterface),{
    crate::sinfo![f; "Triggered!"];
});

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

//     println!("Trying {}", function!());
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
//     crate::sinfo![f; "Triggered!"];
// });

// FName * __thiscall FName::FName(FName *this,wchar_t *param_1,EFindName param_2)

// #[repr(u8)]
// #[derive(Debug, Clone, Copy, PartialEq)]
// pub enum EFindName {
//     Find = 0,          // Only return existing names
//     Add = 1,           // Add to name pool if missing
//     ReplaceNotSafe = 2 // Rarely used
// }

#[cfg(feature = "dev")]
define_pattern_resolver!(FNameCtorWchar, Simple, [
        "48 89 5C 24 ?? 57 48 83 EC 30 48 8B D9 48 89 54 24 ??",
    ]);

#[cfg(feature = "dev")]
CREATE_HOOK!(FNameCtorWchar, INACTIVE,  *mut FName, (this: *mut FName, Str: *const u16, findname: EFindName),{
    // crate::sinfo![f; "Triggered!"];
    unsafe {
        if findname == EFindName::Find {
            let fstring = FString::from(
                widestring::U16CString::from_ptr_str(Str)
                .as_slice_with_nul());
            if fstring.to_string().contains("ModMarker") {
                serror!(f; "FNameCtorWchar: {}", fstring);
            }
        }
    }
});

#[cfg(feature = "dev")]
define_pattern_resolver!(GetAsset,["40 53 48 83 EC 60 48 8B D9 33 D2"]);
#[cfg(feature = "dev")]
CREATE_HOOK!(GetAsset, INACTIVE, *mut UObject, (asset_data: *mut FAssetData),{
    // unsafe {
    //     // crate::sinfo![f; "Triggered! {}", &*asset_data];
    // }
});

#[cfg(feature = "dev")]
define_pattern_resolver!(StaticFindObject,["48 89 5C 24 08 48 89 74 24 18 55 57 41 54 41 56 41 57 48 8B EC 48 83 EC 60 80 3D 98 BB"]);
// UObject* StaticFindObject( UClass* ObjectClass, UObject* InObjectPackage, const TCHAR* OrigInName, bool ExactClass )
CREATE_HOOK!(StaticFindObject, INACTIVE, *mut UObject, (
    obj_class: *mut c_void, 
    package: *mut c_void, 
    name: *const u16, 
    exact_class: bool
), {
    if !name.is_null() {
        let name_str = unsafe { widestring::U16CStr::from_ptr_str(name) };
        crate::sinfo!(f; "StaticFindObject looking for: {}", name_str.to_string_lossy());
    }
});

define_pattern_resolver!(GetDefaultObject,["40 53 48 83 EC 20 48 8B 81 18 01 00 00 48 8B D9 48 85 C0 ?? ?? 84 D2 ?? ?? 48 8B 01 ?? ?? ?? ?? ?? ??"]);
// UObject* GetDefaultObject(bool bCreateIfNeeded = true) const
CREATE_HOOK!(GetDefaultObject, INACTIVE, *mut UObject, (this_ptr: *mut UClass, create_if_needed: bool),{
    crate::sinfo![f; "Triggered!"];
});

define_pattern_resolver!(GetAllActorsOfClass,["40 55 53 56 57 48 8D 6C 24 C1 48 81 EC A8 00 00 00 48 8B 05 70 B1 70 02 48 33 C4 48 89 45 1F 41"]);
// static void GetAllActorsOfClass(const UObject* WorldContextObject, TSubclassOf<AActor> ActorClass, TArray<AActor*>& OutActors);
CREATE_HOOK!(GetAllActorsOfClass, INACTIVE, (WorldContextObject: *const UObject, ActorClass: *mut UClass, OutActors: *mut TArray<*mut UObject>),{
    crate::sinfo![f;
    "Triggered!"];
});

define_pattern_resolver!(StaticLoadObject,["40 55 53 56 57 41 54 41 55 41 56 41 57 48 8D AC 24 28 FA FF FF 48 81 EC D8 06 00 00 48 8B 05 25 5E 9F 03 48 33 C4 48 89 85 C0 05 00 00 48"]);
// UObject* StaticLoadObject(UClass* ObjectClass, UObject* InOuter, const TCHAR* InName, const TCHAR* Filename, uint32 LoadFlags, UPackageMap* Sandbox, bool bAllowObjectReconciliation, FUObjectSerializeContext* InSerializeContext)
// UObject* StaticLoadObject(UClass* Class, UObject* InOuter, const TCHAR* Name, const TCHAR* Filename, uint32 LoadFlags, UPackageMap* Sandbox, bool bAllowNativeComponentClass, const FLinkerInstancingContext* InstancingContext)
CREATE_HOOK!(StaticLoadObject, INACTIVE, *mut UObject, (
    obj_class: *mut c_void, 
    in_outer: *mut c_void, 
    name: *const u16, 
    filename: *const u16, 
    load_flags: u32, 
    sandbox: *mut c_void, 
    allow_native_comp: bool,
    instancing_context: *mut c_void
), {
    if !name.is_null() {
        let name_str = unsafe { U16CStr::from_ptr_str(name) };
        crate::swarn!(f; "StaticLoadObject triggered for: {}", name_str.to_string_lossy());
    }
});