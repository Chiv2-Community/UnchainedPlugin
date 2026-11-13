//! Minimal header-only parser for the game's unversioned cooked UE 4.25 package
//! format. Its whole reason to exist: detect marker (`DA_ModMarker`,
//! `DA_ModMarker_V2`, `DA_MapInfo`, `DA_MapMarker_V2`)
//! data-asset *instances* by reading ONLY the package summary + name/import/
//! export tables — never the object payloads — so discovery can cheaply filter
//! what to register into the Asset Registry at startup.
//!
//! Notes learned while reverse-engineering (see the `mod_loading-8wa` epic):
//! - These packages are **unversioned cooked** (`FileVersionUE4 == 0`), so the
//!   version is taken as VER_LATEST; the `uasset`/`unreal_asset` crates reject
//!   them, hence this hand-rolled parser and no dependency.
//! - Tables live at summary offsets (`NameOffset`/`ImportOffset`/`ExportOffset`),
//!   not inline.
//! - Markers are Blueprint data assets: the class (`DA_ModMarker_C`) is a
//!   `BlueprintGeneratedClass` *export* inside the same package, and the
//!   instance's `ClassIndex` points at it (a positive export index). So class
//!   resolution is a two-pass over the export table, resolving both import and
//!   in-package (export) class references.
//! - Stored name strings carry a trailing NUL (part of the length), which we strip.
//!   A negative length means UTF-16 and counts code units, not bytes.
//! - An export table entry is 104 bytes (bools are serialized as 4 bytes). Both
//!   fixtures confirm it: `DependsOffset - ExportOffset == 104` for one export.

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn i32(&mut self) -> Option<i32> {
        let s = self.data.get(self.pos..self.pos + 4)?;
        self.pos += 4;
        Some(i32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn u32(&mut self) -> Option<u32> {
        self.i32().map(|v| v as u32)
    }
    fn i64(&mut self) -> Option<i64> {
        let s = self.data.get(self.pos..self.pos + 8)?;
        self.pos += 8;
        Some(i64::from_le_bytes(s.try_into().ok()?))
    }
    /// FName reference: index(i32) then number(i32).
    fn name_ref(&mut self) -> Option<(i32, i32)> {
        let index = self.i32()?;
        let number = self.i32()?;
        Some((index, number))
    }
    /// Name/string entry: int32 len then the characters. A positive len counts
    /// bytes; a negative len counts UTF-16 code units (2 bytes each).
    /// A trailing NUL (part of the stored length) is stripped.
    fn name_string(&mut self) -> Option<String> {
        let len = self.i32()?;
        let wide = len < 0;
        let chars = len.unsigned_abs() as usize;
        let byte_len = if wide { chars.checked_mul(2)? } else { chars };
        let bytes = self.data.get(self.pos..self.pos.checked_add(byte_len)?)?;
        self.pos += byte_len;
        let mut s = if wide {
            let units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|w| u16::from_le_bytes([w[0], w[1]]))
                .collect();
            String::from_utf16_lossy(&units)
        } else {
            String::from_utf8_lossy(bytes).into_owned()
        };
        while s.ends_with('\0') {
            s.pop();
        }
        Some(s)
    }
}

/// Smallest serialized size of one name entry (empty string + hash), one import
/// and one export. Used to reject table counts the file cannot hold.
const MIN_NAME_ENTRY: usize = 8;
const IMPORT_ENTRY: usize = 28;
const EXPORT_ENTRY: usize = 104;

/// Turns a table count read from the header into a usable length, or `None`
/// when the file is too small to hold that many entries. The counts come from
/// third-party paks, so they must not size an allocation unchecked.
fn table_len(count: i32, entry_size: usize, data_len: usize) -> Option<usize> {
    let count = count.max(0) as usize;
    (count.checked_mul(entry_size)? <= data_len).then_some(count)
}

/// Parse a cooked package, resolving each export's class name where it is an
/// import or in-package class reference. Returns (names, imports, exports) with
/// each export as (object_name, Option<class_name>). Class is `None` for exports
/// whose class isn't a resolved reference (e.g. native classes / class defs).
#[allow(dead_code)]
fn parse_headers(data: &[u8]) -> Option<(Vec<String>, Vec<String>, Vec<(String, Option<String>)>)> {
    let mut p = Cursor { data, pos: 0 };
    if p.u32()? != 0x9E2A83C1 {
        return None;
    }
    let legacy = p.i32()?;
    if legacy != -6 && legacy != -7 {
        return None;
    }
    let _ue3 = p.i32()?;
    let _file_version_ue4 = p.i32()?;
    let _licensee = p.i32()?;
    let num_custom = p.i32()?;
    p.pos += (num_custom.max(0) as usize) * 20; // FCustomVersion (Optimized) = 20 bytes
    let _total_header_size = p.i32()?;
    let _folder = p.name_string()?;
    let package_flags = p.u32()?;
    let has_editor_data = (package_flags & 0x8000_0000) == 0;

    let name_count = p.i32()?;
    let name_offset = p.i32()?;
    if has_editor_data {
        let _ = p.name_string()?; // localization id
    }
    let _gather_count = p.i32()?;
    let _gather_offset = p.i32()?;
    let export_count = p.i32()?;
    let export_offset = p.i32()?;
    let import_count = p.i32()?;
    let import_offset = p.i32()?;

    let name_count = table_len(name_count, MIN_NAME_ENTRY, data.len())?;
    let import_count = table_len(import_count, IMPORT_ENTRY, data.len())?;
    let export_count = table_len(export_count, EXPORT_ENTRY, data.len())?;

    // Names.
    let mut names = Vec::with_capacity(name_count);
    if name_offset > 0 {
        p.pos = name_offset as usize;
        for _ in 0..name_count {
            names.push(p.name_string()?);
            let _hash = p.u32()?;
        }
    }

    // Imports (cooked form: class_package name, class_name name, outer i32, object name).
    let mut import_names: Vec<String> = Vec::with_capacity(import_count);
    if import_offset > 0 {
        p.pos = import_offset as usize;
        for _ in 0..import_count {
            let _cpk = p.name_ref()?;
            let _cls = p.name_ref()?;
            let _outer = p.i32()?;
            let obj = p.name_ref()?;
            import_names.push(names.get(obj.0 as usize).cloned().unwrap_or_default());
        }
    }

    // Exports (first pass: raw class_index + object name), then resolve classes.
    // One entry is EXPORT_ENTRY (104) bytes: FArchive writes each bool as a
    // 4-byte value.
    let mut raw: Vec<(i32, String)> = Vec::with_capacity(export_count);
    if export_offset > 0 {
        p.pos = export_offset as usize;
        for _ in 0..export_count {
            let class_index = p.i32()?;
            let _super = p.i32()?;
            let _template = p.i32()?;
            let _outer = p.i32()?;
            let obj_ref = p.name_ref()?;
            let _flags = p.u32()?;
            let _ = p.i64()?; // serial size
            let _ = p.i64()?; // serial offset
            let _ = p.u32()?; // bForcedExport
            let _ = p.u32()?; // bNotForClient
            let _ = p.u32()?; // bNotForServer
            p.pos += 16; // PackageGuid
            let _ = p.u32()?; // PackageFlags
            let _ = p.u32()?; // bNotAlwaysLoadedForEditorGame
            let _ = p.u32()?; // bIsAsset
            p.pos += 20; // 5 x i32 deps
            let object_name = names.get(obj_ref.0 as usize).cloned().unwrap_or_default();
            raw.push((class_index, object_name));
        }
    }

    // Second pass: FPackageIndex < 0 → import (-1 - idx); > 0 → export (idx - 1); 0 → none.
    let exports: Vec<(String, Option<String>)> = raw
        .iter()
        .map(|&(ci, ref oname)| {
            let class = if ci < 0 {
                import_names.get((-1 - ci) as usize).cloned()
            } else if ci > 0 {
                raw.get((ci - 1) as usize).map(|(_, n)| n.clone())
            } else {
                None
            };
            (oname.clone(), class)
        })
        .collect();

    Some((names, import_names, exports))
}

/// Which marker data asset an instance class belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerKind {
    /// `DA_ModMarker_C` (deprecated, hard references).
    ModMarkerV1,
    /// `DA_ModMarker_V2_C` (soft references).
    ModMarkerV2,
    /// `DA_MapInfo_C`.
    MapInfo,
    /// `DA_MapMarker_V2_C`.
    MapMarkerV2,
}

/// Maps an instance class name (e.g. `DA_ModMarker_V2_C`) to its marker kind.
/// Case-insensitive, tolerating the `_C`/`_c` class suffix and trailing NUL.
/// The match is exact, so V1 and V2 never shadow each other.
pub fn marker_kind(class_name: &str) -> Option<MarkerKind> {
    let lower = class_name.trim_end_matches('\0').trim_end().to_ascii_lowercase();
    let base = lower.strip_suffix("_c").unwrap_or(&lower);
    match base {
        "da_modmarker" => Some(MarkerKind::ModMarkerV1),
        "da_modmarker_v2" => Some(MarkerKind::ModMarkerV2),
        "da_mapinfo" => Some(MarkerKind::MapInfo),
        "da_mapmarker_v2" => Some(MarkerKind::MapMarkerV2),
        _ => None,
    }
}

/// True if `class_name` is any marker class.
fn is_marker_class(class_name: &str) -> bool {
    marker_kind(class_name).is_some()
}

/// True for a class default object (`Default__<Class>`). The package that
/// defines a marker Blueprint class holds one next to the class export; it is
/// not a marker instance and must not get the package registered.
fn is_class_default_object(object_name: &str) -> bool {
    object_name.starts_with("Default__")
}

/// Returns one `(instance_name, class_name)` per marker instance found in the
/// package header (`instance_name` is the export object name, `class_name` is
/// e.g. `DA_ModMarker_C`). Empty if the package contains no marker instance.
pub fn marker_instances(data: &[u8]) -> Vec<(String, String)> {
    let Some((_, _, exports)) = parse_headers(data) else {
        return Vec::new();
    };
    exports
        .into_iter()
        .filter(|(obj, _)| !is_class_default_object(obj))
        .filter_map(|(obj, class)| class.filter(|c| is_marker_class(c)).map(|c| (obj, c)))
        .collect()
}

/// True if the package contains at least one marker instance.
pub fn is_marker_package(data: &[u8]) -> bool {
    !marker_instances(data).is_empty()
}

/// Number of marker instances per kind in one package header.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MarkerCounts {
    pub mod_markers_v1: usize,
    pub mod_markers_v2: usize,
    pub map_infos: usize,
    pub map_markers_v2: usize,
}

impl MarkerCounts {
    /// True if the package holds at least one marker instance of any kind.
    pub fn any(&self) -> bool {
        self.mod_markers_v1 + self.mod_markers_v2 + self.map_infos + self.map_markers_v2 > 0
    }
}

/// Counts the marker instances of each kind in the package header.
pub fn marker_counts(data: &[u8]) -> MarkerCounts {
    let mut counts = MarkerCounts::default();
    let Some((_, _, exports)) = parse_headers(data) else {
        return counts;
    };
    for (obj, class) in exports {
        if is_class_default_object(&obj) {
            continue;
        }
        match class.as_deref().and_then(marker_kind) {
            Some(MarkerKind::ModMarkerV1) => counts.mod_markers_v1 += 1,
            Some(MarkerKind::ModMarkerV2) => counts.mod_markers_v2 += 1,
            Some(MarkerKind::MapInfo) => counts.map_infos += 1,
            Some(MarkerKind::MapMarkerV2) => counts.map_markers_v2 += 1,
            None => {}
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        let path = format!("{}/test-resources/cooked_header/{}", env!("CARGO_MANIFEST_DIR"), name);
        std::fs::read(path).expect("fixture should exist")
    }

    #[test]
    fn detects_da_modmarker_instance() {
        let data = fixture("NihiMaps_Marker.uasset");
        let hits = marker_instances(&data);
        assert!(
            hits.iter().any(|(_, c)| is_marker_class(c)),
            "expected a marker instance, got {:?}",
            hits
        );
    }

    #[test]
    fn detects_da_mapinfo_instance() {
        let data = fixture("NihiMaps_MapInfo.uasset");
        let hits = marker_instances(&data);
        assert!(
            hits.iter().any(|(_, c)| is_marker_class(c)),
            "expected a DA_MapInfo instance, got {:?}",
            hits
        );
    }

    #[test]
    fn not_a_marker_package_returns_empty() {
        // A name table alone (truncated/non-package) must not panic and returns empty.
        let data = vec![0u8; 0];
        assert!(marker_instances(&data).is_empty());
        assert!(!is_marker_package(&data));
    }
    /// Byte offsets of the table counts in a `synthetic_package_with` header
    /// (tag 4 + version 4 + 16 + header size 4 + folder name 9 + flags 4 = 41).
    const SYNTHETIC_NAME_COUNT_AT: usize = 41;
    const SYNTHETIC_EXPORT_COUNT_AT: usize = 57;
    const SYNTHETIC_IMPORT_COUNT_AT: usize = 65;

    /// Builds a minimal cooked header with one export named `instance` whose
    /// class is an import named `class_name` (how a marker instance references
    /// a Blueprint class that lives in another package).
    fn synthetic_package(instance: &str, class_name: &str) -> Vec<u8> {
        synthetic_package_with(&["None", class_name, instance], &[1], &[(-1, 2)])
    }

    /// Builds a minimal cooked header. `imports` lists the name index of each
    /// import's object name; `exports` lists `(class_index, name index)` per
    /// export, with `class_index` as an FPackageIndex (negative = import,
    /// positive = export). Non-ASCII names are stored as UTF-16.
    fn synthetic_package_with(names: &[&str], imports: &[i32], exports: &[(i32, i32)]) -> Vec<u8> {
        fn name(out: &mut Vec<u8>, s: &str) {
            if s.is_ascii() {
                out.extend(((s.len() + 1) as i32).to_le_bytes());
                out.extend(s.as_bytes());
                out.push(0);
            } else {
                let units: Vec<u16> = s.encode_utf16().chain([0]).collect();
                out.extend((-(units.len() as i32)).to_le_bytes());
                for unit in units {
                    out.extend(unit.to_le_bytes());
                }
            }
        }
        let mut out = Vec::new();
        out.extend(0x9E2A83C1u32.to_le_bytes());
        out.extend((-7i32).to_le_bytes());
        out.extend([0u8; 16]); // ue3, ue4, licensee, custom version count
        out.extend(0i32.to_le_bytes()); // total header size
        name(&mut out, "None"); // folder name
        out.extend(0x8000_0000u32.to_le_bytes()); // PKG_FilterEditorOnly
        out.extend((names.len() as i32).to_le_bytes());
        let name_offset_at = out.len();
        out.extend([0u8; 12]); // name offset, gatherable count + offset
        out.extend((exports.len() as i32).to_le_bytes());
        let export_offset_at = out.len();
        out.extend([0u8; 4]);
        out.extend((imports.len() as i32).to_le_bytes());
        let import_offset_at = out.len();
        out.extend([0u8; 4]);

        let patch = |out: &mut Vec<u8>, at: usize| {
            let here = (out.len() as i32).to_le_bytes();
            out[at..at + 4].copy_from_slice(&here);
        };
        patch(&mut out, name_offset_at);
        for n in names {
            name(&mut out, n);
            out.extend(0u32.to_le_bytes()); // hash
        }
        patch(&mut out, import_offset_at);
        for object_name in imports {
            out.extend([0u8; 16]); // class package + class name refs
            out.extend(0i32.to_le_bytes()); // outer
            out.extend(object_name.to_le_bytes());
            out.extend(0i32.to_le_bytes());
        }
        patch(&mut out, export_offset_at);
        for (class_index, object_name) in exports {
            let entry_start = out.len();
            out.extend(class_index.to_le_bytes());
            out.extend([0u8; 12]); // super, template, outer
            out.extend(object_name.to_le_bytes());
            out.extend(0i32.to_le_bytes());
            // flags, serial size + offset, 3 bools, guid, package flags, 2 bools, 5 deps
            out.extend([0u8; 4 + 8 + 8 + 12 + 16 + 4 + 8 + 20]);
            assert_eq!(out.len() - entry_start, EXPORT_ENTRY);
        }
        out
    }

    #[test]
    fn synthetic_count_offsets_match_the_builder() {
        let data = synthetic_package_with(&["None", "A", "B"], &[1, 1], &[(-1, 2)]);
        let count_at = |at: usize| i32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        assert_eq!(count_at(SYNTHETIC_NAME_COUNT_AT), 3);
        assert_eq!(count_at(SYNTHETIC_EXPORT_COUNT_AT), 1);
        assert_eq!(count_at(SYNTHETIC_IMPORT_COUNT_AT), 2);
    }

    #[test]
    fn finds_a_marker_that_is_not_the_first_export() {
        // The Blueprint class is defined in the same package (export 1) and the
        // marker instance (export 2) points at it with a positive class index.
        let data = synthetic_package_with(
            &["None", "BlueprintGeneratedClass", "DA_ModMarker_V2_C", "Quarry_ModMarker"],
            &[1],
            &[(-1, 2), (1, 3)],
        );
        assert_eq!(
            marker_instances(&data),
            vec![("Quarry_ModMarker".to_string(), "DA_ModMarker_V2_C".to_string())]
        );
        assert_eq!(marker_counts(&data), MarkerCounts { mod_markers_v2: 1, ..Default::default() });
    }

    #[test]
    fn class_definition_package_is_not_a_marker_package() {
        let data = synthetic_package_with(
            &["None", "BlueprintGeneratedClass", "DA_ModMarker_V2_C", "Default__DA_ModMarker_V2_C"],
            &[1],
            &[(-1, 2), (1, 3)],
        );
        assert!(marker_instances(&data).is_empty());
        assert!(!marker_counts(&data).any());
    }

    #[test]
    fn reads_utf16_names_of_odd_and_even_length() {
        // "Mä" + NUL is 3 UTF-16 units, "Märk" + NUL is 5, "Mäh" + NUL is 4.
        for instance in ["Mä", "Märk", "Mäh"] {
            let data = synthetic_package(instance, "DA_ModMarker_C");
            assert_eq!(
                marker_instances(&data),
                vec![(instance.to_string(), "DA_ModMarker_C".to_string())]
            );
        }
    }

    #[test]
    fn truncated_utf16_name_is_skipped_without_panicking() {
        let mut data = synthetic_package("Mä", "DA_ModMarker_C");
        data.truncate(data.len() - EXPORT_ENTRY - IMPORT_ENTRY - 5);
        assert!(marker_instances(&data).is_empty());
    }

    #[test]
    fn oversized_table_counts_are_rejected() {
        for count_at in [SYNTHETIC_NAME_COUNT_AT, SYNTHETIC_EXPORT_COUNT_AT, SYNTHETIC_IMPORT_COUNT_AT] {
            let mut data = synthetic_package("Thing_Marker", "DA_ModMarker_C");
            data[count_at..count_at + 4].copy_from_slice(&i32::MAX.to_le_bytes());
            assert!(parse_headers(&data).is_none());
            assert!(!marker_counts(&data).any());
        }
        assert_eq!(table_len(3, 8, 24), Some(3));
        assert_eq!(table_len(4, 8, 24), None);
        assert_eq!(table_len(-5, 8, 24), Some(0));
    }

    #[test]
    fn marker_kind_keeps_versions_apart() {
        assert_eq!(marker_kind("DA_ModMarker_C"), Some(MarkerKind::ModMarkerV1));
        assert_eq!(marker_kind("DA_ModMarker_V2_C"), Some(MarkerKind::ModMarkerV2));
        assert_eq!(marker_kind("da_modmarker_v2_c\0"), Some(MarkerKind::ModMarkerV2));
        assert_eq!(marker_kind("DA_MapInfo_C"), Some(MarkerKind::MapInfo));
        assert_eq!(marker_kind("DA_MapMarker_V2_C"), Some(MarkerKind::MapMarkerV2));
        assert_eq!(marker_kind("DA_ModMarker_V3_C"), None);
        assert_eq!(marker_kind("DA_MapMarker_C"), None);
    }

    #[test]
    fn counts_v2_marker_instances() {
        let data = synthetic_package("Quarry_ModMarker", "DA_ModMarker_V2_C");
        assert_eq!(
            marker_instances(&data),
            vec![("Quarry_ModMarker".to_string(), "DA_ModMarker_V2_C".to_string())]
        );
        let counts = marker_counts(&data);
        assert_eq!(counts, MarkerCounts { mod_markers_v2: 1, ..Default::default() });
        assert!(counts.any());

        let data = synthetic_package("Quarry_MapMarker", "DA_MapMarker_V2_C");
        assert_eq!(marker_counts(&data), MarkerCounts { map_markers_v2: 1, ..Default::default() });
    }

    #[test]
    fn real_v1_fixtures_count_as_v1_only() {
        let counts = marker_counts(&fixture("NihiMaps_Marker.uasset"));
        assert!(counts.mod_markers_v1 > 0);
        assert_eq!(counts.mod_markers_v2, 0);
        assert!(marker_counts(&fixture("NihiMaps_MapInfo.uasset")).map_infos > 0);
        assert!(!marker_counts(&synthetic_package("Thing", "SomeOther_C")).any());
    }
}
