#[repr(C, align(8))]
pub struct FConfigFile {
    pairs: [u8; 0x50],
    pub(crate) dirty: u8,
    pub(crate) no_save: u8,
    padding: [u8; 6],
    name: super::FName,
    source_ini_hierarchy: [u8; 0x58],
    source_engine_config_dir: [u8; 0x10],
    source_project_config_dir: [u8; 0x10],
    pub(crate) source_config_file: *mut FConfigFile,
    tail: [u8; 0x60],
}

const _: () = {
    assert!(std::mem::size_of::<FConfigFile>() == 0x140);
    assert!(std::mem::offset_of!(FConfigFile, no_save) == 0x51);
    assert!(std::mem::offset_of!(FConfigFile, source_ini_hierarchy) == 0x60);
    assert!(std::mem::offset_of!(FConfigFile, source_config_file) == 0xd8);
};
