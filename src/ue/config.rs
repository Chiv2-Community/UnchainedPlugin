use std::mem::ManuallyDrop;

use super::{FName, FString, TMap, TMultiMap};

#[repr(C)]
struct FConfigValue {
    saved_value: ManuallyDrop<FString>,
    expanded_value: ManuallyDrop<FString>,
}

#[repr(C)]
struct FConfigSection {
    pairs: ManuallyDrop<TMultiMap<FName, FConfigValue>>,
    array_of_struct_keys: ManuallyDrop<TMap<FName, FString>>,
}

#[repr(C)]
struct FIniFilename {
    filename: ManuallyDrop<FString>,
    required: u8,
    cache_key: ManuallyDrop<FString>,
}

#[repr(C)]
struct FConfigFileHierarchy {
    entries: ManuallyDrop<TMap<i32, FIniFilename>>,
    key_gen: i32,
}

// UE's native destructor owns all container/string members, not Rust's field drop glue.
#[repr(C, align(8))]
pub struct FConfigFile {
    pairs: ManuallyDrop<TMap<FString, FConfigSection>>,
    pub(crate) dirty: u8,
    pub(crate) no_save: u8,
    padding: [u8; 6],
    name: FName,
    source_ini_hierarchy: FConfigFileHierarchy,
    source_engine_config_dir: ManuallyDrop<FString>,
    source_project_config_dir: ManuallyDrop<FString>,
    pub(crate) source_config_file: *mut FConfigFile,
    // Remaining target-specific members have not been modeled.
    opaque_tail: [u8; 0x60],
}

const _: () = {
    assert!(std::mem::size_of::<FConfigValue>() == 0x20);
    assert!(std::mem::size_of::<FConfigSection>() == 0xa0);
    assert!(std::mem::offset_of!(FConfigSection, array_of_struct_keys) == 0x50);
    assert!(std::mem::size_of::<FIniFilename>() == 0x28);
    assert!(std::mem::offset_of!(FIniFilename, cache_key) == 0x18);
    assert!(std::mem::size_of::<FConfigFileHierarchy>() == 0x58);
    assert!(std::mem::offset_of!(FConfigFileHierarchy, key_gen) == 0x50);
    assert!(std::mem::size_of::<FConfigFile>() == 0x140);
    assert!(std::mem::align_of::<FConfigFile>() == 8);
    assert!(!std::mem::needs_drop::<FConfigFile>());
    assert!(std::mem::offset_of!(FConfigFile, dirty) == 0x50);
    assert!(std::mem::offset_of!(FConfigFile, no_save) == 0x51);
    assert!(std::mem::offset_of!(FConfigFile, source_ini_hierarchy) == 0x60);
    assert!(std::mem::offset_of!(FConfigFile, source_engine_config_dir) == 0xb8);
    assert!(std::mem::offset_of!(FConfigFile, source_project_config_dir) == 0xc8);
    assert!(std::mem::offset_of!(FConfigFile, source_config_file) == 0xd8);
    assert!(std::mem::offset_of!(FConfigFile, opaque_tail) == 0xe0);
};
