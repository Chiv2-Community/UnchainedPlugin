//! Minimal header-only parser for the game's unversioned cooked UE 4.25 package
//! format. Its whole reason to exist: detect `DA_ModMarker` / `DA_MapInfo`
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

/// Marker data classes we care about for the main menu.
pub const MARKER_CLASSES: [&str; 2] = ["DA_ModMarker", "DA_MapInfo"];

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

/// True if `class_name` (e.g. `DA_ModMarker_C`) is a marker class (case-
/// insensitive, tolerating the `_C`/`_c` class suffix and trailing NUL).
fn is_marker_class(class_name: &str) -> bool {
    let mut base = class_name.trim_end_matches('\0').trim_end().to_ascii_lowercase();
    if let Some(stripped) = base.strip_suffix("_c") {
        base = stripped.to_string();
    }
    MARKER_CLASSES.iter().any(|m| base == m.to_ascii_lowercase())
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

/// Returns which marker class an instance class name belongs to.
fn marker_kind(class: &str) -> Option<&'static str> {
    let mut base = class.trim_end_matches('\0').trim_end().to_ascii_lowercase();
    if let Some(s) = base.strip_suffix("_c") {
        base = s.to_string();
    }
    match base.as_str() {
        "da_modmarker" => Some("DA_ModMarker"),
        "da_mapinfo" => Some("DA_MapInfo"),
        _ => None,
    }
}

/// Counts of `DA_ModMarker` vs `DA_MapInfo` instances in the package header.
/// Returns `(mod_markers, map_infos)`.
pub fn marker_counts(data: &[u8]) -> (usize, usize) {
    let Some((_, _, exports)) = parse_headers(data) else {
        return (0, 0);
    };
    let mut mods = 0usize;
    let mut infos = 0usize;
    for (_, class) in exports {
        match class.as_deref().and_then(marker_kind) {
            Some("DA_ModMarker") => mods += 1,
            Some("DA_MapInfo") => infos += 1,
            _ => {}
        }
    }
    (mods, infos)
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
}
