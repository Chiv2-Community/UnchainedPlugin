#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverrideLayer {
    Map,
    MapIndex,
    MapProfile,
}

impl OverrideLayer {
    pub fn dir(self) -> &'static str {
        match self {
            Self::Map => "maps",
            Self::MapIndex => "indices",
            Self::MapProfile => "profiles",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerSelection {
    pub layer: OverrideLayer,
    pub slot: String,
}

impl LayerSelection {
    pub fn new(layer: OverrideLayer, slot: impl Into<String>) -> Self {
        Self {
            layer,
            slot: slot.into(),
        }
    }
}

/// A single Windows directory component, never a path supplied by a map URL.
pub fn valid_component(value: &str) -> bool {
    valid_name(value, false)
}

fn valid_name(value: &str, wildcards: bool) -> bool {
    let stem = value.split('.').next().unwrap().to_ascii_uppercase();
    !value.is_empty()
        && value.len() <= 128
        && value != "."
        && value != ".."
        && !value.ends_with('.')
        && value
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.') || (wildcards && c == '~'))
        && !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        && !stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|suffix| {
                matches!(
                    suffix,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
}

fn valid_slot(layer: OverrideLayer, slot: &str) -> bool {
    valid_name(slot, layer != OverrideLayer::MapIndex)
}

/// Match anchored literals separated by zero-or-more-character wildcards.
fn wildcard_match(pattern: &str, target: &str) -> bool {
    let Some((prefix, rest)) = pattern.split_once('~') else {
        return pattern == target;
    };
    let Some(mut remaining) = target.strip_prefix(prefix) else {
        return false;
    };
    let (middle, suffix) = rest.rsplit_once('~').unwrap_or(("", rest));
    for part in middle.split('~') {
        let Some(position) = remaining.find(part) else {
            return false;
        };
        remaining = &remaining[position + part.len()..];
    }
    remaining.ends_with(suffix)
}

/// Windows-safe wildcards: fewer literal characters first, exact match last.
pub fn matching_slots(layer: OverrideLayer, target: &str, folders: &[String]) -> Vec<String> {
    if !valid_component(target) {
        return Vec::new();
    }
    let target_lower = target.to_ascii_lowercase();
    let mut matches: Vec<_> = folders
        .iter()
        .filter(|folder| {
            valid_slot(layer, folder) && wildcard_match(&folder.to_ascii_lowercase(), &target_lower)
        })
        .cloned()
        .collect();
    matches.sort_by_key(|folder| {
        (
            !folder.contains('~'),
            folder.chars().filter(|c| *c != '~').count(),
            folder.to_ascii_lowercase(),
            folder.clone(),
        )
    });
    matches.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    // Preserve direct exact-path lookup even when directory enumeration failed.
    if !matches
        .iter()
        .any(|folder| folder.eq_ignore_ascii_case(target))
    {
        matches.push(target.to_owned());
    }
    matches
}

pub fn expand_layers(root: &str, layers: &[LayerSelection]) -> Vec<LayerSelection> {
    let mut expanded = Vec::new();
    for selection in layers {
        if selection.layer == OverrideLayer::MapIndex {
            expanded.push(selection.clone());
            continue;
        }
        let directory = std::path::Path::new(root)
            .join("ini-overrides")
            .join(selection.layer.dir());
        let folders = std::fs::read_dir(directory)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect::<Vec<_>>();
        expanded.extend(
            matching_slots(selection.layer, &selection.slot, &folders)
                .into_iter()
                .map(|slot| LayerSelection::new(selection.layer, slot)),
        );
    }
    expanded
}

pub fn map_folder(url: &str) -> Option<String> {
    let normalized = url.split('?').next()?.replace('\\', "/");
    let leaf = normalized.rsplit('/').next()?;
    // Accept both package paths and object paths such as /Game/Maps/Foo.Foo.
    let leaf = leaf.split('.').next()?;
    valid_component(leaf).then(|| leaf.to_owned())
}

pub fn config_filename(filename: &str) -> Option<String> {
    let normalized = filename.replace('\\', "/");
    let leaf = normalized.rsplit('/').next()?;
    let (category, extension) = leaf.rsplit_once('.')?;
    (extension.eq_ignore_ascii_case("ini") && valid_component(category)).then(|| leaf.to_owned())
}

pub fn build_profile_override_path(
    root: &str,
    layer: OverrideLayer,
    slot: &str,
    config: &str,
) -> String {
    format!(
        "{}/ini-overrides/{}/{slot}/{config}",
        root.trim_end_matches(['/', '\\']),
        layer.dir()
    )
    .replace('\\', "/")
}

pub fn build_override_paths(root: &str, config: &str, layers: &[LayerSelection]) -> Vec<String> {
    if config_filename(config).as_deref() != Some(config) {
        return Vec::new();
    }
    let mut paths = Vec::new();
    for selection in layers {
        if !valid_slot(selection.layer, &selection.slot) {
            continue;
        }
        paths.push(build_profile_override_path(
            root,
            selection.layer,
            &selection.slot,
            config,
        ));
    }
    paths
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MapContext {
    pub map: Option<String>,
    pub index: Option<i32>,
}

impl MapContext {
    pub fn new(map: &str, index: Option<i32>) -> Self {
        Self {
            map: map_folder(map),
            index: index.filter(|i| *i >= 0),
        }
    }
}

/// Lowest precedence first; a profile never replaces the index layer.
pub fn select_layers(context: &MapContext, profile: Option<&str>) -> Vec<LayerSelection> {
    let mut layers = Vec::new();
    if let Some(map) = &context.map {
        layers.push(LayerSelection::new(OverrideLayer::Map, map));
    }
    if let Some(index) = context.index {
        if let Some(profile) = profile.filter(|profile| valid_component(profile)) {
            layers.push(LayerSelection::new(OverrideLayer::MapProfile, profile));
        }
        layers.push(LayerSelection::new(
            OverrideLayer::MapIndex,
            index.to_string(),
        ));
    }
    layers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn destination_map_is_a_validated_leaf() {
        assert_eq!(
            map_folder("/Game/Maps/TDM_QuarryKP?NumBots=12").as_deref(),
            Some("TDM_QuarryKP")
        );
        assert_eq!(
            map_folder(r"\Game\Maps\TDM_QuarryKP.TDM_QuarryKP").as_deref(),
            Some("TDM_QuarryKP")
        );
        for invalid in ["", "../", "..", "/Game/Maps/", "C:Foo", "Foo\0Bar", "CON"] {
            assert_eq!(map_folder(invalid), None, "{invalid:?}");
        }
    }

    #[test]
    fn rejects_profile_traversal_and_windows_aliases() {
        for invalid in [
            "../other",
            r"..\other",
            ".",
            "..",
            "x:y",
            "foo?x",
            "NUL.ini",
            "LPT1",
            "foo.",
        ] {
            assert!(!valid_component(invalid), "{invalid:?}");
        }
        assert!(valid_component("night_tdm-2"));
    }

    #[test]
    fn rejects_superscript_device_names_but_preserves_unicode() {
        for prefix in ["COM", "cOm", "LPT", "lPt"] {
            for digit in ["1", "9", "¹", "²", "³"] {
                for extension in ["", ".ini", ".extra.ini"] {
                    let name = format!("{prefix}{digit}{extension}");
                    assert!(!valid_component(&name), "{name}");
                    assert!(!valid_slot(OverrideLayer::Map, &name));
                    assert!(!valid_slot(OverrideLayer::MapProfile, &name));
                }
            }
        }
        for valid in ["COM10", "COM¹-extra", "LPT10", "LPT²-extra", "夜間"] {
            assert!(valid_component(valid), "{valid}");
        }
    }

    #[test]
    fn no_fallback_categories_or_unsafe_slots() {
        assert!(build_override_paths(
            "C:/P",
            "Game.ini",
            &[LayerSelection::new(OverrideLayer::Map, "../escape")]
        )
        .is_empty());
        for invalid in ["../Game.ini", r"C:\Game.ini", "Game.yaml", "COM¹.ini"] {
            assert!(build_override_paths(
                "C:/P",
                invalid,
                &[LayerSelection::new(OverrideLayer::Map, "Quarry")]
            )
            .is_empty());
        }
        assert_eq!(
            config_filename(r"C:\Saved\WindowsServer\Game.ini").as_deref(),
            Some("Game.ini")
        );
        assert_eq!(config_filename("Game.yaml"), None);
    }

    #[test]
    fn new_destination_never_inherits_an_old_rotation_index() {
        let old = MapContext::new("/Game/Maps/A", Some(3));
        let new = MapContext::new("/Game/Maps/B", None);
        assert_eq!(old.index, Some(3));
        assert_eq!(new.index, None);
        assert_eq!(MapContext::new("B", Some(-1)).index, None);
    }

    #[test]
    fn indices_override_profiles_which_override_maps() {
        let context = MapContext::new("Quarry", Some(2));
        assert_eq!(
            select_layers(&context, Some("busy")),
            vec![
                LayerSelection::new(OverrideLayer::Map, "Quarry"),
                LayerSelection::new(OverrideLayer::MapProfile, "busy"),
                LayerSelection::new(OverrideLayer::MapIndex, "2"),
            ]
        );
    }

    #[test]
    fn missing_or_invalid_profile_keeps_index_layer() {
        let context = MapContext::new("Quarry", Some(2));
        for profile in [None, Some("../escape")] {
            assert_eq!(
                select_layers(&context, profile),
                vec![
                    LayerSelection::new(OverrideLayer::Map, "Quarry"),
                    LayerSelection::new(OverrideLayer::MapIndex, "2"),
                ]
            );
        }
        assert_eq!(
            select_layers(&MapContext::new("Quarry", None), Some("busy")),
            vec![LayerSelection::new(OverrideLayer::Map, "Quarry")]
        );
    }

    #[test]
    fn wildcard_maps_and_profiles_merge_broad_to_specific_then_exact() {
        let folders = [
            "FFA_Courtyard",
            "ffa_courtyard",
            "FFA_~",
            "ffa_~",
            "F~",
            "~",
            "TDM_~",
            "FFA_*",
            "../~",
        ]
        .map(String::from);
        for layer in [OverrideLayer::Map, OverrideLayer::MapProfile] {
            assert_eq!(
                matching_slots(layer, "FFA_Courtyard", &folders),
                ["~", "F~", "FFA_~", "FFA_Courtyard"]
            );
            assert_eq!(
                matching_slots(layer, "ffa_other", &folders),
                ["~", "F~", "FFA_~", "ffa_other"]
            );
            assert_eq!(
                matching_slots(layer, "FFA_", &folders),
                ["~", "F~", "FFA_~", "FFA_"]
            );
        }
    }

    #[test]
    fn suffix_middle_and_multiple_wildcards_match_without_overlapping_literals() {
        for (pattern, target, expected) in [
            ("~Courtyard", "FFA_Courtyard", true),
            ("~Courtyard", "FFA_Courtyard_extra", false),
            ("FFA~yard", "FFA_Courtyard", true),
            ("FFA~yard", "TDM_Courtyard", false),
            ("~FFA~Court~yard~", "FFA_Courtyard", true),
            ("FFA~~Courtyard", "FFA_Courtyard", true),
            ("F~F", "F", false),
            ("~ab~bc", "abc", false),
            ("~ab~bc", "ababc", true),
            ("~Court~FFA~", "FFA_Courtyard", false),
            ("~~~", "FFA_Courtyard", true),
            ("夜~間", "夜間", true),
            ("夜~間", "夜の時間", true),
        ] {
            assert_eq!(
                wildcard_match(pattern, target),
                expected,
                "{pattern}: {target}"
            );
        }
        let folders = [
            "~yard",
            "FFA~yard",
            "~",
            "FFA_~",
            "ffa~YARD",
            "FFA_Courtyard",
        ]
        .map(String::from);
        for layer in [OverrideLayer::Map, OverrideLayer::MapProfile] {
            assert_eq!(
                matching_slots(layer, "FFA_Courtyard", &folders),
                ["~", "FFA_~", "~yard", "FFA~yard", "FFA_Courtyard"]
            );
        }
    }

    #[test]
    fn indices_are_exact_and_wildcard_paths_are_validated() {
        assert_eq!(
            matching_slots(
                OverrideLayer::MapIndex,
                "2",
                &["~".into(), "2~".into(), "2".into()]
            ),
            ["2"]
        );
        for invalid in ["../~", "foo/~", "foo\\~", "foo*", "CON", "foo~.", "foo?~"] {
            assert!(!valid_slot(OverrideLayer::Map, invalid), "{invalid}");
        }
        for valid in ["~yard", "FFA~yard", "~FFA~yard~", "foo~~", "CON~"] {
            assert!(valid_slot(OverrideLayer::Map, valid), "{valid}");
            assert!(!valid_slot(OverrideLayer::MapIndex, valid), "{valid}");
        }
        assert!(matching_slots(OverrideLayer::Map, "../bad", &[]).is_empty());
        let paths = build_override_paths(
            "P",
            "Game.ini",
            &[
                LayerSelection::new(OverrideLayer::Map, "FFA_~"),
                LayerSelection::new(OverrideLayer::MapProfile, "busy~"),
                LayerSelection::new(OverrideLayer::MapIndex, "2~"),
            ],
        );
        assert_eq!(
            paths,
            [
                "P/ini-overrides/maps/FFA_~/Game.ini",
                "P/ini-overrides/profiles/busy~/Game.ini"
            ]
        );
    }

    #[test]
    fn filesystem_expansion_preserves_layer_precedence() {
        let root = std::env::temp_dir().join(format!("unchained-wildcards-{}", std::process::id()));
        for folder in [
            "maps/F~",
            "maps/FFA_~",
            "maps/~Courtyard",
            "maps/F~Court~yard",
            "maps/FFA_Courtyard",
            "profiles/b~",
            "profiles/busy",
            "indices/2~",
        ] {
            std::fs::create_dir_all(root.join("ini-overrides").join(folder)).unwrap();
        }
        let layers = select_layers(&MapContext::new("FFA_Courtyard", Some(2)), Some("busy"));
        let expanded = expand_layers(root.to_str().unwrap(), &layers);
        assert_eq!(
            expanded,
            vec![
                LayerSelection::new(OverrideLayer::Map, "F~"),
                LayerSelection::new(OverrideLayer::Map, "FFA_~"),
                LayerSelection::new(OverrideLayer::Map, "~Courtyard"),
                LayerSelection::new(OverrideLayer::Map, "F~Court~yard"),
                LayerSelection::new(OverrideLayer::Map, "FFA_Courtyard"),
                LayerSelection::new(OverrideLayer::MapProfile, "b~"),
                LayerSelection::new(OverrideLayer::MapProfile, "busy"),
                LayerSelection::new(OverrideLayer::MapIndex, "2"),
            ]
        );
        for config in ["Engine.ini", "Game.ini"] {
            let paths = build_override_paths(root.to_str().unwrap(), config, &expanded);
            assert_eq!(paths.len(), expanded.len());
            for (path, selection) in paths.iter().zip(&expanded) {
                assert_eq!(
                    path,
                    &format!(
                        "{}/ini-overrides/{}/{}/{config}",
                        root.display().to_string().replace('\\', "/"),
                        selection.layer.dir(),
                        selection.slot
                    )
                );
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
