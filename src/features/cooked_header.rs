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

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn u8(&mut self) -> Option<u8> {
        let b = *self.data.get(self.pos)?;
        self.pos += 1;
        Some(b)
    }
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
    /// Name/string entry: int32 len then bytes (negative len = wide / UTF-16).
    /// A trailing NUL (part of the stored length) is stripped.
    fn name_string(&mut self) -> Option<String> {
        let len = self.i32()?;
        let abs = len.unsigned_abs() as usize;
        let wide = len < 0;
        let bytes = self.data.get(self.pos..self.pos + abs)?;
        self.pos += abs;
        let mut s = if wide {
            let mut s = String::new();
            for w in bytes.chunks(2) {
                let c = u16::from_le_bytes([w[0], w[1]]);
                s.push(char::from_u32(c as u32).unwrap_or('\u{FFFD}'));
            }
            s
        } else {
            String::from_utf8_lossy(bytes).into_owned()
        };
        while s.ends_with('\0') {
            s.pop();
        }
        Some(s)
    }
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

    // Names.
    let mut names = Vec::with_capacity(name_count.max(0) as usize);
    if name_offset > 0 {
        p.pos = name_offset as usize;
        for _ in 0..name_count.max(0) {
            names.push(p.name_string()?);
            let _hash = p.u32()?;
        }
    }

    // Imports (cooked form: class_package name, class_name name, outer i32, object name).
    let mut import_names: Vec<String> = Vec::with_capacity(import_count.max(0) as usize);
    if import_offset > 0 {
        p.pos = import_offset as usize;
        for _ in 0..import_count.max(0) {
            let _cpk = p.name_ref()?;
            let _cls = p.name_ref()?;
            let _outer = p.i32()?;
            let obj = p.name_ref()?;
            import_names.push(names.get(obj.0 as usize).cloned().unwrap_or_default());
        }
    }

    // Exports (first pass: raw class_index + object name), then resolve classes.
    let mut raw: Vec<(i32, String)> = Vec::with_capacity(export_count.max(0) as usize);
    if export_offset > 0 {
        p.pos = export_offset as usize;
        for _ in 0..export_count.max(0) {
            let class_index = p.i32()?;
            let _super = p.i32()?;
            let _template = p.i32()?;
            let _outer = p.i32()?;
            let obj_ref = p.name_ref()?;
            let _flags = p.u32()?;
            let _ = p.i64()?; // serial size
            let _ = p.i64()?; // serial offset
            let _ = p.u8()?; // bForcedExport
            let _ = p.u8()?; // bNotForClient
            let _ = p.u8()?; // bNotForServer
            p.pos += 16; // PackageGuid
            let _ = p.u32()?; // PackageFlags
            let _ = p.u8()?; // bNotAlwaysLoadedForEditorGame
            let _ = p.u8()?; // bIsAsset
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

/// Returns one `(instance_name, class_name)` per marker instance found in the
/// package header (`instance_name` is the export object name, `class_name` is
/// e.g. `DA_ModMarker_C`). Empty if the package contains no marker instance.
pub fn marker_instances(data: &[u8]) -> Vec<(String, String)> {
    let Some((_, _, exports)) = parse_headers(data) else {
        return Vec::new();
    };
    exports
        .into_iter()
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
    for (_, class) in exports {
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
    /// Builds a minimal cooked header with one export named `instance` whose
    /// class is an import named `class_name` (how a marker instance references
    /// a Blueprint class that lives in another package).
    fn synthetic_package(instance: &str, class_name: &str) -> Vec<u8> {
        fn name(out: &mut Vec<u8>, s: &str) {
            out.extend(((s.len() + 1) as i32).to_le_bytes());
            out.extend(s.as_bytes());
            out.push(0);
        }
        let names = ["None", class_name, instance];
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
        out.extend(1i32.to_le_bytes());
        let export_offset_at = out.len();
        out.extend([0u8; 4]);
        out.extend(1i32.to_le_bytes());
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
        out.extend([0u8; 16]); // class package + class name refs
        out.extend(0i32.to_le_bytes()); // outer
        out.extend(1i32.to_le_bytes()); // object name = names[1]
        out.extend(0i32.to_le_bytes());
        patch(&mut out, export_offset_at);
        out.extend((-1i32).to_le_bytes()); // class index = import 0
        out.extend([0u8; 12]); // super, template, outer
        out.extend(2i32.to_le_bytes()); // object name = names[2]
        out.extend(0i32.to_le_bytes());
        out.extend([0u8; 4 + 8 + 8 + 3 + 16 + 4 + 2 + 20]);
        out
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
