use std::os::raw::c_void;

use crate::{game::engine::{FSoftObjectPath, FSoftObjectPtr, TSoftClassPtr}, ue::{FField, FProperty, FString, TArray, TMap, TSet, UClass, UObject, UStruct}};

#[repr(C)]
pub struct DA_ModMarker_C {
    _private: [u8; 0x30], 
    pub mod_actors: TMap<*mut UClass, FString>,    // Offset 0x0030
    pub custom_objects: TMap<*mut UObject, FString>, // Offset 0x0080
}

/// `DA_ModMarker_V2_C` as dumped from the game (size 0x142). The live layout is
/// resolved by property name (see [`V2MarkerOffsets`]); this struct supplies the
/// fallback offsets. Only ever used through a pointer to a live engine object.
#[repr(C)]
pub struct DA_ModMarker_V2_C {
    _private: [u8; 0x30],
    pub mod_id: FString,                          // 0x0030
    pub mod_version: FString,                     // 0x0040
    pub format_version: i32,                      // 0x0050
    _padding_0: [u8; 0x4],                        // 0x0054
    pub icon: FSoftObjectPtr,                     // 0x0058 TSoftObjectPtr<UTexture2D>
    pub list_tags: TArray<FString>,               // 0x0080
    pub mod_actor_classes: TSet<FSoftObjectPath>, // 0x0090 TSet<FSoftClassPath>
    pub bundled_markers: TSet<FSoftObjectPath>,   // 0x00E0
    pub excluded_paths: TArray<FString>,          // 0x0130
    pub own_pak: bool,                            // 0x0140
    pub apply_version_to_bundled: bool,           // 0x0141
}
const _: () = {
    use std::mem::offset_of;
    assert!(offset_of!(DA_ModMarker_V2_C, mod_id) == 0x30);
    assert!(offset_of!(DA_ModMarker_V2_C, mod_version) == 0x40);
    assert!(offset_of!(DA_ModMarker_V2_C, format_version) == 0x50);
    assert!(offset_of!(DA_ModMarker_V2_C, icon) == 0x58);
    assert!(offset_of!(DA_ModMarker_V2_C, list_tags) == 0x80);
    assert!(offset_of!(DA_ModMarker_V2_C, mod_actor_classes) == 0x90);
    assert!(offset_of!(DA_ModMarker_V2_C, bundled_markers) == 0xE0);
    assert!(offset_of!(DA_ModMarker_V2_C, excluded_paths) == 0x130);
    assert!(offset_of!(DA_ModMarker_V2_C, own_pak) == 0x140);
    assert!(offset_of!(DA_ModMarker_V2_C, apply_version_to_bundled) == 0x141);
};

/// Size of `DA_ModMarker_V2_C` in the dump; a class at least this large may
/// use the dump offsets when a property cannot be found by name.
pub const V2_MARKER_DUMP_SIZE: usize = 0x142;

/// One marker field the loader reads: property name, expected field type and
/// its offset in the dump.
struct V2Field {
    name: &'static str,
    type_name: &'static str,
    dump_offset: usize,
}

/// Picks the offset to use for one field. `found` is the by-name lookup result
/// (`None` if the property is missing). Returns `Err` when the field cannot be
/// read safely, and a note to log when the result deserves attention.
fn pick_v2_offset(
    field: &V2Field,
    found: Option<&crate::ue::FoundProperty>,
    properties_size: usize,
    dump_size: usize,
) -> Result<(usize, Option<String>), String> {
    match found {
        Some(property) if property.type_name != field.type_name => Err(format!(
            "{} is a {}, expected {}",
            field.name, property.type_name, field.type_name
        )),
        Some(property) if property.offset != field.dump_offset => Ok((
            property.offset,
            Some(format!(
                "{} is at {:#x}, the dump has {:#x}; using the live offset",
                field.name, property.offset, field.dump_offset
            )),
        )),
        Some(property) => Ok((property.offset, None)),
        None if properties_size >= dump_size => Ok((
            field.dump_offset,
            Some(format!(
                "{} not found by name; using dump offset {:#x}",
                field.name, field.dump_offset
            )),
        )),
        None => Err(format!(
            "{} not found by name and the class is too small ({:#x}) for the dump layout",
            field.name, properties_size
        )),
    }
}

/// Resolves one marker field on the live class: by property name, else the
/// dump offset (see [`pick_v2_offset`]). Notes are logged as warnings.
///
/// # Safety
/// `class` must be a live `UClass`.
unsafe fn resolve_marker_field(
    class: *const UClass,
    marker_name: &str,
    dump_size: usize,
    field: V2Field,
) -> Result<usize, String> {
    let properties_size = (*class).ustruct.properties_size.max(0) as usize;
    let found = crate::ue::find_property(class, field.name);
    let (offset, note) = pick_v2_offset(&field, found.as_ref(), properties_size, dump_size)?;
    if let Some(note) = note {
        crate::swarn!("{}: {}", marker_name, note);
    }
    Ok(offset)
}

/// Offsets of the `DA_ModMarker_V2_C` fields the loader reads, resolved from
/// the live class by property name with the dump layout as fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct V2MarkerOffsets {
    mod_id: usize,
    mod_version: usize,
    format_version: usize,
    mod_actor_classes: usize,
    bundled_markers: usize,
}

impl V2MarkerOffsets {
    /// Resolves the offsets from a marker's class. `Err` means the class does
    /// not look like a V2 marker the loader can read.
    ///
    /// # Safety
    /// `class` must be a live `UClass`.
    pub unsafe fn resolve(class: *const UClass) -> Result<Self, String> {
        use std::mem::offset_of;
        if class.is_null() {
            return Err("marker has no class".into());
        }
        let resolve_field = |name, type_name, dump_offset| {
            resolve_marker_field(class, "DA_ModMarker_V2", V2_MARKER_DUMP_SIZE, V2Field { name, type_name, dump_offset })
        };
        Ok(Self {
            mod_id: resolve_field("ModId", "StrProperty", offset_of!(DA_ModMarker_V2_C, mod_id))?,
            mod_version: resolve_field("ModVersion", "StrProperty", offset_of!(DA_ModMarker_V2_C, mod_version))?,
            format_version: resolve_field("FormatVersion", "IntProperty", offset_of!(DA_ModMarker_V2_C, format_version))?,
            mod_actor_classes: resolve_field("ModActorClasses", "SetProperty", offset_of!(DA_ModMarker_V2_C, mod_actor_classes))?,
            bundled_markers: resolve_field("BundledMarkers", "SetProperty", offset_of!(DA_ModMarker_V2_C, bundled_markers))?,
        })
    }

    /// # Safety
    /// `marker` must be a live instance of the class these offsets came from,
    /// and `T` must be the field's real type.
    unsafe fn field<'a, T>(marker: *const UObject, offset: usize) -> &'a T {
        &*marker.cast::<u8>().add(offset).cast::<T>()
    }

    /// # Safety
    /// `marker` must be a live instance of the class these offsets came from.
    pub unsafe fn mod_id(&self, marker: *const UObject) -> String {
        Self::field::<FString>(marker, self.mod_id).to_string()
    }

    /// # Safety
    /// See [`Self::mod_id`].
    pub unsafe fn mod_version(&self, marker: *const UObject) -> String {
        Self::field::<FString>(marker, self.mod_version).to_string()
    }

    /// # Safety
    /// See [`Self::mod_id`].
    pub unsafe fn format_version(&self, marker: *const UObject) -> i32 {
        *Self::field::<i32>(marker, self.format_version)
    }

    /// Soft class paths of the mod actors, unset entries dropped.
    ///
    /// # Safety
    /// See [`Self::mod_id`].
    pub unsafe fn mod_actor_classes(&self, marker: *const UObject) -> Vec<String> {
        Self::paths(Self::field(marker, self.mod_actor_classes))
    }

    /// Object paths of the markers bundled into this marker's pak.
    ///
    /// # Safety
    /// See [`Self::mod_id`].
    pub unsafe fn bundled_markers(&self, marker: *const UObject) -> Vec<String> {
        Self::paths(Self::field(marker, self.bundled_markers))
    }

    fn paths(set: &TSet<FSoftObjectPath>) -> Vec<String> {
        set.iter()
            .map(FSoftObjectPath::to_path_string)
            .filter(|path| !path.is_empty())
            .collect()
    }
}

/// Size of a UE4.25 `FProperty` (FField 0x38 + members through the four trailing
/// property-link pointers). `FArrayProperty::Inner` and `FStructProperty::Struct`
/// each follow the full `FProperty`, so both sit at this offset from the field.
const FPROPERTY_SIZE: usize = 0x78;

/// Raw `TArray` header {data, num, max} as laid out in memory. Only ever read as
/// a view over engine memory - never wrapped in `TArray<T>`, whose `Drop` would
/// free the engine's buffer.
#[repr(C)]
struct ArrayHeader {
    data: *const u8,
    num: i32,
    max: i32,
}

/// One `F_ShippedMap` row read from a marker's `Maps` array. `DA_MapMarker_V2`'s
/// `Maps` is a `TArray<F_ShippedMap>` (a Blueprint UserDefinedStruct); its members
/// are resolved by authored name at runtime (see [`MapMarkerOffsets::resolve`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapMarkerRow {
    /// Object path of the map (empty rows are dropped by `rows`).
    pub map_path: String,
    /// `State == Held`: kept in the marker but excluded from shipping.
    pub held: bool,
    /// Per-map loading-screen texture path; empty when the row sets none.
    pub loading_screen: String,
    /// Authored map name (summary); empty when unset or no AR tag was baked.
    pub map_name: String,
    /// Authored map description (summary); empty when unset.
    pub description: String,
    pub gamemode: i32,
    pub faction: i32,
}

/// Property field by name on `start` or its supers, as the raw `FField` pointer
/// (unlike [`crate::ue::find_property`], which returns only offset + type name).
///
/// # Safety
/// `start` must be null or a live `UStruct` (a `UClass` cast to `UStruct`).
unsafe fn find_field(start: *const UStruct, name: &str) -> Option<*const FField> {
    const MAX_FIELDS: usize = 4096;
    let mut current = start;
    let mut visited = 0usize;
    while !current.is_null() {
        let mut field = (*current).child_properties;
        while !field.is_null() {
            visited += 1;
            if visited > MAX_FIELDS {
                return None;
            }
            if (*field).name_private.to_string() == name {
                return Some(field);
            }
            field = (*field).next;
        }
        current = (*current).super_struct;
    }
    None
}

/// Member byte offsets inside a `UserDefinedStruct`, keyed by authored field name
/// (the token before the first `_` in each member's mangled FName, e.g.
/// `MapName_11_<GUID>` -> `MapName`).
///
/// # Safety
/// `strukt` must be null or a live `UScriptStruct` (read as its `UStruct` prefix).
unsafe fn struct_member_offsets(strukt: *const UStruct) -> std::collections::HashMap<String, usize> {
    const MAX_FIELDS: usize = 256;
    let mut out = std::collections::HashMap::new();
    if strukt.is_null() {
        return out;
    }
    let mut field = (*strukt).child_properties;
    let mut visited = 0usize;
    while !field.is_null() {
        visited += 1;
        if visited > MAX_FIELDS {
            break;
        }
        let raw = (*field).name_private.to_string();
        let token = raw.split('_').next().unwrap_or(&raw).to_string();
        let offset = (*field.cast::<FProperty>()).offset_internal;
        if offset >= 0 {
            out.entry(token).or_insert(offset as usize);
        }
        field = (*field).next;
    }
    out
}

/// Offsets the loader reads from a `DA_MapMarker_V2` instance: the class-level
/// `DefaultLoadingScreen` and `Maps` array, plus the element stride and each
/// `F_ShippedMap` member offset, all resolved from the live class/struct by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapMarkerOffsets {
    default_loading_screen: usize,
    maps: usize,
    element_size: usize,
    m_map: Option<usize>,
    m_state: Option<usize>,
    m_loading_screen: Option<usize>,
    m_map_name: Option<usize>,
    m_description: Option<usize>,
    m_gamemode: Option<usize>,
    m_faction: Option<usize>,
}

impl MapMarkerOffsets {
    /// Resolves the offsets from a marker's class. `Err` means the class does
    /// not look like a V2 map marker the loader can read.
    ///
    /// # Safety
    /// `class` must be a live `UClass`.
    pub unsafe fn resolve(class: *const UClass) -> Result<Self, String> {
        if class.is_null() {
            return Err("marker has no class".into());
        }
        // DefaultLoadingScreen (soft Texture2D), required.
        let dls = crate::ue::find_property(class, "DefaultLoadingScreen")
            .ok_or_else(|| "DefaultLoadingScreen not found".to_string())?;
        if dls.type_name != "SoftObjectProperty" {
            return Err(format!("DefaultLoadingScreen is a {}, expected SoftObjectProperty", dls.type_name));
        }
        // Maps (TArray<F_ShippedMap>), required.
        let maps_field = find_field(class.cast::<UStruct>(), "Maps")
            .ok_or_else(|| "Maps array not found - not a V2 map marker".to_string())?;
        let maps_offset = {
            let offset = (*maps_field.cast::<FProperty>()).offset_internal;
            if offset < 0 {
                return Err("Maps has a negative offset".into());
            }
            offset as usize
        };
        // FArrayProperty::Inner (a StructProperty) follows the full FProperty.
        let inner = *(maps_field.cast::<u8>().add(FPROPERTY_SIZE).cast::<*const FProperty>());
        if inner.is_null() {
            return Err("Maps array has no inner property".into());
        }
        let element_size = (*inner).element_size.max(0) as usize;
        if element_size == 0 {
            return Err("Maps element size is zero".into());
        }
        // FStructProperty::Struct follows the full FProperty.
        let strukt = *((inner as *const u8).add(FPROPERTY_SIZE).cast::<*const UStruct>());
        let members = struct_member_offsets(strukt);
        let member = |name: &str| members.get(name).copied();
        let offsets = Self {
            default_loading_screen: dls.offset,
            maps: maps_offset,
            element_size,
            m_map: member("Map"),
            m_state: member("State"),
            m_loading_screen: member("LoadingScreen"),
            m_map_name: member("MapName"),
            m_description: member("MapDescription"),
            m_gamemode: member("GamemodeType"),
            m_faction: member("AttackingFaction"),
        };
        if offsets.m_map.is_none() {
            return Err("F_ShippedMap has no Map member".into());
        }
        Ok(offsets)
    }

    /// Texture path of the marker's default loading screen; empty if unset.
    ///
    /// # Safety
    /// `marker` must be a live instance of the class these offsets came from.
    pub unsafe fn default_loading_screen(&self, marker: *const UObject) -> String {
        let texture = &*marker.cast::<u8>().add(self.default_loading_screen).cast::<FSoftObjectPtr>();
        texture.path.to_path_string()
    }

    /// Every `F_ShippedMap` row in the marker's `Maps` array (Held rows included;
    /// the caller excludes them). Rows with no map path are dropped.
    ///
    /// # Safety
    /// See [`Self::default_loading_screen`].
    pub unsafe fn rows(&self, marker: *const UObject) -> Vec<MapMarkerRow> {
        let header = &*marker.cast::<u8>().add(self.maps).cast::<ArrayHeader>();
        let num = header.num.max(0) as usize;
        let map_offset = match self.m_map {
            Some(offset) => offset,
            None => return Vec::new(),
        };
        if header.data.is_null() || self.element_size == 0 {
            return Vec::new();
        }
        let string_at = |elem: *const u8, offset: Option<usize>| -> String {
            offset.map_or_else(String::new, |o| (*elem.add(o).cast::<FString>()).to_string())
        };
        let i32_at = |elem: *const u8, offset: Option<usize>| -> i32 {
            offset.map_or(0, |o| *elem.add(o).cast::<i32>())
        };
        let mut rows = Vec::with_capacity(num);
        for index in 0..num {
            let elem = match index.checked_mul(self.element_size) {
                Some(step) => header.data.add(step),
                None => break,
            };
            let map_path = (*elem.add(map_offset).cast::<FSoftObjectPath>()).to_path_string();
            if map_path.is_empty() {
                continue;
            }
            rows.push(MapMarkerRow {
                map_path,
                held: self.m_state.is_some_and(|o| *elem.add(o).cast::<u8>() != 0),
                loading_screen: self.m_loading_screen.map_or_else(String::new, |o| {
                    (*elem.add(o).cast::<FSoftObjectPtr>()).path.to_path_string()
                }),
                map_name: string_at(elem, self.m_map_name),
                description: string_at(elem, self.m_description),
                gamemode: i32_at(elem, self.m_gamemode),
                faction: i32_at(elem, self.m_faction),
            });
        }
        rows
    }
}

#[repr(C)]
pub struct ArgonSDKModBase {
    pub actor_padding: [u8; 0x258],
    // 0x0258
    pub uber_graph_frame: u64, 
    // 0x0260
    pub default_scene_root: *mut c_void, 
    // 0x0268
    pub mod_name: FString,
    // 0x0278
    pub mod_version: FString,
    // 0x0288
    pub mod_description: FString,
    // 0x0298
    pub author: FString,
    // 0x02A8
    pub b_silent_load: bool,
    pub padding_0: [u8; 0x7], // 0x02A9
    // 0x02B0
    pub mod_repo_url: FString,
    // 0x02C0
    pub duplicate: bool,
    pub b_enable_by_default: bool,
    pub b_show_in_gui: bool,
    pub b_online_only: bool,
    pub b_host_only: bool,
    pub b_allow_on_frontend: bool,
    pub padding_1: [u8; 0x2], // 0x02C6
    // 0x02C8
    pub mod_version_repl: FString,
    // 0x02D8
    // pub ingame_mod_menu_widgets: TMap<FString, *mut c_void>,
    // 0x0328
    // pub b_clientside: bool,
}

#[repr(C)]
#[derive(Debug)]
pub struct UModLoaderSettings_C {
    // 0x0000 (size 0x28) 
    pub base: [u8; 0x28], 
    // 0x0028 (size: 0x8)
    pub uber_graph_frame: u64, 
    // 0x0030 (size: 0x10)
    pub last_active_mods: TArray<FString>,
    // 0x0040 (size: 0x10)
    pub last_active_map: FString,
    // 0x0050 (size: 0x10)
    pub last_active_category: FString,
    // 0x0060 (size: 0x01)
    pub b_enable_bots: bool,
    // 0x0061 (size: 0x03) - Padding
    pub padding_0: [u8; 0x3],
    // 0x0064 (size: 0x04)
    pub num_player_bots: i32,
    // 0x0068 (size: 0x10)
    pub enabled_mods: TArray<TSoftClassPtr>,
    // 0x0078 (size: 0x01)
    pub b_use_player_bots: bool,
    // 0x0079 (size: 0x01)
    pub b_show_gui: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ue::FoundProperty;

    const MOD_ID: V2Field = V2Field { name: "ModId", type_name: "StrProperty", dump_offset: 0x30 };

    fn found(offset: usize, type_name: &str) -> FoundProperty {
        FoundProperty { offset, type_name: type_name.into() }
    }

    #[test]
    fn live_offset_matching_the_dump_is_silent() {
        let property = found(0x30, "StrProperty");
        assert_eq!(pick_v2_offset(&MOD_ID, Some(&property), 0x142, 0x142), Ok((0x30, None)));
    }

    #[test]
    fn live_offset_wins_over_the_dump_with_a_note() {
        let property = found(0x48, "StrProperty");
        let (offset, note) = pick_v2_offset(&MOD_ID, Some(&property), 0x160, 0x142).unwrap();
        assert_eq!(offset, 0x48);
        assert!(note.is_some());
    }

    #[test]
    fn wrong_field_type_is_rejected() {
        let property = found(0x30, "ArrayProperty");
        assert!(pick_v2_offset(&MOD_ID, Some(&property), 0x142, 0x142).is_err());
    }

    #[test]
    fn missing_property_falls_back_only_when_the_class_is_large_enough() {
        let (offset, note) = pick_v2_offset(&MOD_ID, None, 0x142, 0x142).unwrap();
        assert_eq!(offset, 0x30);
        assert!(note.is_some());
        assert!(pick_v2_offset(&MOD_ID, None, 0x141, 0x142).is_err());
    }
}
