//! Native source: TBL/Source/TBL/TBLGameModule.cpp:33 (shipping PDB).
//! FTBLGameModule is source-local; no declaring header is assumed.

// Legacy key for FTBLGameModule::StartupModule. Installed Steam function RVA
// 0x196df50; the PDB identifies the older homolog at RVA 0x1809e20.
define_pattern_resolver!(AllowAlternateBackend, [
    "48 8B C4 55 48 8D 68 ?? 48 81 EC C0 00 00 00 48 89 58 ?? 48 89 70 ?? 48 89 78 ?? 4C 89 60 ?? 4C 89 70"
]);
