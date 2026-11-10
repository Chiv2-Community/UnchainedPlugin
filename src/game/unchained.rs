use std::os::raw::c_void;

use crate::{game::engine::{FSoftObjectPath, FSoftObjectPtr, TSoftClassPtr}, ue::{FString, TArray, TMap, TMapView, TSet, UClass, UObject}};

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

/// `DA_MapMarker_V2_C` as dumped from the game (size 0x1C8). Like the mod
/// marker, the live layout is resolved by property name (see
/// [`MapMarkerOffsets`]) and this struct supplies the fallback offsets.
#[repr(C)]
pub struct DA_MapMarker_V2_C {
    _private: [u8; 0x30],
    pub notes: FString,                                              // 0x0030
    pub default_loading_screen: FSoftObjectPtr,                      // 0x0040 TSoftObjectPtr<UTexture2D>
    /// Shipped map -> its loading screen texture (unset = use the default).
    pub shipped_map_paths: TMapView<FSoftObjectPath, FSoftObjectPtr>, // 0x0068
    pub ignored_maps: TSet<FSoftObjectPath>,                         // 0x00B8
    pub shipped_map_layers: TSet<FSoftObjectPath>,                   // 0x0108
    pub exclude_folders: TArray<FString>,                            // 0x0158
    pub exclude_name_patterns: TArray<FString>,                      // 0x0168
    pub extra_required_maps: TSet<FSoftObjectPath>,                  // 0x0178
}
const _: () = {
    use std::mem::{offset_of, size_of};
    assert!(offset_of!(DA_MapMarker_V2_C, notes) == 0x30);
    assert!(offset_of!(DA_MapMarker_V2_C, default_loading_screen) == 0x40);
    assert!(offset_of!(DA_MapMarker_V2_C, shipped_map_paths) == 0x68);
    assert!(offset_of!(DA_MapMarker_V2_C, ignored_maps) == 0xB8);
    assert!(offset_of!(DA_MapMarker_V2_C, shipped_map_layers) == 0x108);
    assert!(offset_of!(DA_MapMarker_V2_C, exclude_folders) == 0x158);
    assert!(offset_of!(DA_MapMarker_V2_C, exclude_name_patterns) == 0x168);
    assert!(offset_of!(DA_MapMarker_V2_C, extra_required_maps) == 0x178);
    assert!(size_of::<DA_MapMarker_V2_C>() == MAP_MARKER_DUMP_SIZE);
};

/// Size of `DA_MapMarker_V2_C` in the dump.
pub const MAP_MARKER_DUMP_SIZE: usize = 0x1C8;

/// Offsets of the `DA_MapMarker_V2_C` fields the plugin reads, resolved from
/// the live class by property name with the dump layout as fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapMarkerOffsets {
    default_loading_screen: usize,
    shipped_map_paths: usize,
}

impl MapMarkerOffsets {
    /// Resolves the offsets from a marker's class. `Err` means the class does
    /// not look like a map marker the plugin can read.
    ///
    /// # Safety
    /// `class` must be a live `UClass`.
    pub unsafe fn resolve(class: *const UClass) -> Result<Self, String> {
        use std::mem::offset_of;
        if class.is_null() {
            return Err("marker has no class".into());
        }
        let resolve_field = |name, type_name, dump_offset| {
            resolve_marker_field(class, "DA_MapMarker_V2", MAP_MARKER_DUMP_SIZE, V2Field { name, type_name, dump_offset })
        };
        Ok(Self {
            default_loading_screen: resolve_field(
                "DefaultLoadingScreen", "SoftObjectProperty", offset_of!(DA_MapMarker_V2_C, default_loading_screen))?,
            shipped_map_paths: resolve_field(
                "ShippedMapPaths", "MapProperty", offset_of!(DA_MapMarker_V2_C, shipped_map_paths))?,
        })
    }

    /// Texture path of the marker's default loading screen; empty if unset.
    ///
    /// # Safety
    /// `marker` must be a live instance of the class these offsets came from.
    pub unsafe fn default_loading_screen(&self, marker: *const UObject) -> String {
        let texture = &*marker.cast::<u8>().add(self.default_loading_screen).cast::<FSoftObjectPtr>();
        texture.path.to_path_string()
    }

    /// `(map object path, loading screen texture path)` per shipped map; the
    /// texture path is empty when the entry has none. Unset map paths are dropped.
    ///
    /// # Safety
    /// See [`Self::default_loading_screen`].
    pub unsafe fn shipped_map_paths(&self, marker: *const UObject) -> Vec<(String, String)> {
        let maps = &*marker
            .cast::<u8>()
            .add(self.shipped_map_paths)
            .cast::<TMapView<FSoftObjectPath, FSoftObjectPtr>>();
        maps.iter()
            .map(|(map, texture)| (map.to_path_string(), texture.path.to_path_string()))
            .filter(|(map, _)| !map.is_empty())
            .collect()
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
