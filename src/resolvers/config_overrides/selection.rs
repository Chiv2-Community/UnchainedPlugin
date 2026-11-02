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

/// A single Windows directory component, never a path supplied by a map URL.
pub fn valid_component(value: &str) -> bool {
    let stem = value.split('.').next().unwrap().to_ascii_uppercase();
    !value.is_empty()
        && value.len() <= 128
        && value != "."
        && value != ".."
        && !value.ends_with('.')
        && value
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
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
    valid_component(slot)
        || (layer != OverrideLayer::MapIndex
            && slot.strip_suffix('~').is_some_and(|prefix| {
                slot.len() <= 128 && (prefix.is_empty() || valid_component(prefix))
            }))
}

/// Windows-safe prefix wildcards: broadest first, exact match last.
pub fn matching_slots(layer: OverrideLayer, target: &str, folders: &[String]) -> Vec<String> {
    if !valid_component(target) {
        return Vec::new();
    }
    let target_lower = target.to_ascii_lowercase();
    let mut matches: Vec<_> = folders
        .iter()
        .filter(|folder| {
            valid_slot(layer, folder)
                && match folder.strip_suffix('~') {
                    Some(prefix) => {
                        layer != OverrideLayer::MapIndex
                            && target_lower.starts_with(&prefix.to_ascii_lowercase())
                    }
                    None => target.eq_ignore_ascii_case(folder),
                }
        })
        .cloned()
        .collect();
    matches.sort_by_key(|folder| {
        (
            !folder.ends_with('~'),
            folder.chars().count(),
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

pub fn expand_layers(
    root: &str,
    layers: &[(OverrideLayer, String)],
) -> Vec<(OverrideLayer, String)> {
    let mut expanded = Vec::new();
    for (layer, target) in layers {
        if *layer == OverrideLayer::MapIndex {
            expanded.push((*layer, target.clone()));
            continue;
        }
        let directory = std::path::Path::new(root)
            .join("ini-overrides")
            .join(layer.dir());
        let folders = std::fs::read_dir(directory)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .filter_map(|entry| entry.file_name().into_string().ok())
            .collect::<Vec<_>>();
        expanded.extend(
            matching_slots(*layer, target, &folders)
                .into_iter()
                .map(|slot| (*layer, slot)),
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

pub fn build_override_paths(
    root: &str,
    config: &str,
    layers: &[(OverrideLayer, String)],
) -> Vec<String> {
    if config_filename(config).as_deref() != Some(config) {
        return Vec::new();
    }
    let mut paths = Vec::new();
    for (layer, slot) in layers {
        if !valid_slot(*layer, slot) {
            continue;
        }
        paths.push(build_profile_override_path(root, *layer, slot, config));
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
pub fn select_layers(context: &MapContext, profile: Option<&str>) -> Vec<(OverrideLayer, String)> {
    let mut layers = Vec::new();
    if let Some(map) = &context.map {
        layers.push((OverrideLayer::Map, map.clone()));
    }
    if let Some(index) = context.index {
        if let Some(profile) = profile.filter(|profile| valid_component(profile)) {
            layers.push((OverrideLayer::MapProfile, profile.to_owned()));
        }
        layers.push((OverrideLayer::MapIndex, index.to_string()));
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
                    assert!(!valid_slot(OverrideLayer::Map, &format!("{name}~")));
                    assert!(!valid_slot(OverrideLayer::MapProfile, &format!("{name}~")));
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
            &[(OverrideLayer::Map, "../escape".into())]
        )
        .is_empty());
        for invalid in ["../Game.ini", r"C:\Game.ini", "Game.yaml", "COM¹.ini"] {
            assert!(build_override_paths(
                "C:/P",
                invalid,
                &[(OverrideLayer::Map, "Quarry".into())]
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
                (OverrideLayer::Map, "Quarry".into()),
                (OverrideLayer::MapProfile, "busy".into()),
                (OverrideLayer::MapIndex, "2".into()),
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
                    (OverrideLayer::Map, "Quarry".into()),
                    (OverrideLayer::MapIndex, "2".into()),
                ]
            );
        }
        assert_eq!(
            select_layers(&MapContext::new("Quarry", None), Some("busy")),
            vec![(OverrideLayer::Map, "Quarry".into())]
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
            "FFA_~~",
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
    fn indices_are_exact_and_wildcard_paths_are_validated() {
        assert_eq!(
            matching_slots(
                OverrideLayer::MapIndex,
                "2",
                &["~".into(), "2~".into(), "2".into()]
            ),
            ["2"]
        );
        for invalid in ["../~", "foo/~", "foo\\~", "foo~~", "foo*", "CON~"] {
            assert!(!valid_slot(OverrideLayer::Map, invalid), "{invalid}");
        }
        assert!(matching_slots(OverrideLayer::Map, "../bad", &[]).is_empty());
        let paths = build_override_paths(
            "P",
            "Game.ini",
            &[
                (OverrideLayer::Map, "FFA_~".into()),
                (OverrideLayer::MapProfile, "busy~".into()),
                (OverrideLayer::MapIndex, "2~".into()),
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
                (OverrideLayer::Map, "F~".into()),
                (OverrideLayer::Map, "FFA_~".into()),
                (OverrideLayer::Map, "FFA_Courtyard".into()),
                (OverrideLayer::MapProfile, "b~".into()),
                (OverrideLayer::MapProfile, "busy".into()),
                (OverrideLayer::MapIndex, "2".into()),
            ]
        );
        for config in ["Engine.ini", "Game.ini"] {
            let paths = build_override_paths(root.to_str().unwrap(), config, &expanded);
            assert_eq!(paths.len(), expanded.len());
            for (path, (layer, slot)) in paths.iter().zip(&expanded) {
                assert_eq!(
                    path,
                    &format!(
                        "{}/ini-overrides/{}/{slot}/{config}",
                        root.display(),
                        layer.dir()
                    )
                );
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
