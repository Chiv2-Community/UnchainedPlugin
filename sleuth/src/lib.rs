
mod scan;

// println!("cargo:rustc-link-lib=static=windows.0.52.0");
// println!("cargo:rustc-link-search=native={}", find_windows_lib_dir());
// fn find_windows_lib_dir() -> String {
//     let home = std::env::var("USERPROFILE").unwrap(); // Or "HOME" for Unix
//     format!(r"{}\.cargo\registry\src\index.crates.io-6f17d22bba15001f\windows_x86_64_msvc-0.52.6\lib", home)
// }

#[no_mangle]
pub extern "C" fn generate_json() -> u8 {
    println!("test asd");
    let res = scan::scan();

    2+2
}
