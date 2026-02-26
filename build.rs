extern crate winres;

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let mut res = winres::WindowsResource::new();
        
        let version = env!("CARGO_PKG_VERSION");
        let mut components = version.split('.').fuse();
        let major = components.next().and_then(|s| s.parse::<u16>().ok()).unwrap_or(0);
        let minor = components.next().and_then(|s| s.parse::<u16>().ok()).unwrap_or(0);
        let patch = components.next().and_then(|s| s.parse::<u16>().ok()).unwrap_or(0);
        
        res.set_version_info(winres::VersionInfo::FILEVERSION, 
            ((major as u64) << 48) | ((minor as u64) << 32) | ((patch as u64) << 16));
        res.set_version_info(winres::VersionInfo::PRODUCTVERSION, 
            ((major as u64) << 48) | ((minor as u64) << 32) | ((patch as u64) << 16));
        
        res.set("CompanyName", "UnchainedPlugin Contributors");
        res.set("FileDescription", "UnchainedPlugin");
        res.set("FileVersion", version);
        res.set("InternalName", "UnchainedPlugin");
        res.set("LegalCopyright", "Copyright (C) 2023-2025");
        res.set("OriginalFilename", "UnchainedPlugin.dll");
        res.set("ProductName", "UnchainedPlugin");
        res.set("ProductVersion", version);
        
        res.compile().unwrap();
    }
}
