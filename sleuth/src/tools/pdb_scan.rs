// mostly AI slop
#![allow(dead_code)]
use pdb::{FallibleIterator, PDB};
use std::fs::File;
use winapi::um::libloaderapi::GetModuleHandleW;

/// Convert RVA to virtual address given the module base
fn rva_to_va(rva: u32, module_base: usize) -> usize {
    module_base + rva as usize
}

/// List all functions in the PDB with their RVA and VA
pub fn list_functions_with_addresses(pdb_path: &str, module_base: usize) -> pdb::Result<()> {
    let file = File::open(pdb_path)?;
    let mut pdb = PDB::open(file)?;

    let symbol_table = pdb.global_symbols()?;
    let address_map = pdb.address_map()?;

    let mut symbols = symbol_table.iter();
    while let Some(symbol) = symbols.next()? {
        if let Ok(pdb::SymbolData::Public(data)) = symbol.parse() {
            if data.function {
                let rva = data.offset.to_rva(&address_map).unwrap_or_default();
                let va = rva_to_va(rva.into(), module_base);
                println!("Function: {} at RVA {:#X}, VA {:#X}", data.name, rva.0, va);
            }
        }
    }

    Ok(())
}

/// Find the function name containing a given RVA
pub fn get_function_name_from_rva(pdb_path: &str, target_rva: u32) -> pdb::Result<Option<String>> {
    let file = File::open(pdb_path)?;
    let mut pdb = PDB::open(file)?;

    let symbol_table = pdb.global_symbols()?;
    let address_map = pdb.address_map()?;

    let mut symbols = symbol_table.iter();
    while let Some(symbol) = symbols.next()? {
        if let Ok(pdb::SymbolData::Public(data)) = symbol.parse() {
            if data.function {
                let rva = data.offset.to_rva(&address_map).unwrap_or_default();

                // PDB does not store sizes reliably, so assume exact match for now
                if rva == pdb::Rva(target_rva) {
                    return Ok(Some(data.name.to_string().into()));
                }
            }
        }
    }

    Ok(None)
}

/// Example of getting the current module base in your process (Windows)
fn get_current_module_base(module_name: &str) -> usize {
    // Convert Rust &str to null-terminated UTF16 for GetModuleHandleW
    let wide: Vec<u16> = module_name.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { GetModuleHandleW(wide.as_ptr()) as usize }
}

pub fn process_pdb() -> pdb::Result<()> {
    let pdb_file = "example.pdb";

    // Example: current module base (replace DLL name if needed)
    let module_base = get_current_module_base("mydll.dll");
    println!("Module base: {:#X}", module_base);

    // List all functions
    list_functions_with_addresses(pdb_file, module_base)?;

    // Example: get function name for a specific RVA
    let target_rva = 0x1234;
    if let Some(name) = get_function_name_from_rva(pdb_file, target_rva)? {
        println!("Function at RVA {:#X} is {}", target_rva, name);
    } else {
        println!("No function found at RVA {:#X}", target_rva);
    }

    Ok(())
}
