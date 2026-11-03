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
    matches.sort_by_cached_key(|folder| {
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

pub fn config_filename(filename: &str) -> Option<&str> {
    let leaf = filename.rsplit(['/', '\\']).next()?;
    let (category, extension) = leaf.rsplit_once('.')?;
    (extension.eq_ignore_ascii_case("ini") && valid_component(category)).then_some(leaf)
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

pub fn build_override_path(root: &str, config: &str, selection: &LayerSelection) -> Option<String> {
    (config_filename(config) == Some(config) && valid_slot(selection.layer, &selection.slot))
        .then(|| build_profile_override_path(root, selection.layer, &selection.slot, config))
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
    use rstest::rstest;

    #[rstest]
    #[case::package_url("/Game/Maps/TDM_QuarryKP?NumBots=12", Some("TDM_QuarryKP"))]
    #[case::object_path(r"\Game\Maps\TDM_QuarryKP.TDM_QuarryKP", Some("TDM_QuarryKP"))]
    #[case::empty("", None)]
    #[case::parent_path("../", None)]
    #[case::parent_component("..", None)]
    #[case::directory("/Game/Maps/", None)]
    #[case::drive("C:Foo", None)]
    #[case::nul("Foo\0Bar", None)]
    #[case::device("CON", None)]
    fn destination_map_is_a_validated_leaf(#[case] url: &str, #[case] expected: Option<&str>) {
        assert_eq!(map_folder(url).as_deref(), expected);
    }

    #[rstest]
    #[case::parent_path("../other", false)]
    #[case::windows_parent_path(r"..\other", false)]
    #[case::current(".", false)]
    #[case::parent("..", false)]
    #[case::colon("x:y", false)]
    #[case::query("foo?x", false)]
    #[case::device_extension("NUL.ini", false)]
    #[case::device("LPT1", false)]
    #[case::trailing_dot("foo.", false)]
    #[case::profile("night_tdm-2", true)]
    #[case::com_ten("COM10", true)]
    #[case::com_suffix("COM¹-extra", true)]
    #[case::lpt_ten("LPT10", true)]
    #[case::lpt_suffix("LPT²-extra", true)]
    #[case::unicode("夜間", true)]
    fn validates_components(#[case] name: &str, #[case] expected: bool) {
        assert_eq!(valid_component(name), expected);
    }

    #[rstest]
    fn rejects_windows_device_names(
        #[values("COM", "cOm", "LPT", "lPt")] prefix: &str,
        #[values("1", "9", "¹", "²", "³")] digit: &str,
        #[values("", ".ini", ".extra.ini")] extension: &str,
    ) {
        let name = format!("{prefix}{digit}{extension}");
        assert!(!valid_component(&name));
        assert!(!valid_slot(OverrideLayer::Map, &name));
        assert!(!valid_slot(OverrideLayer::MapProfile, &name));
    }

    #[rstest]
    #[case::map(
        OverrideLayer::Map,
        "FFA_~",
        Some("P/ini-overrides/maps/FFA_~/Game.ini")
    )]
    #[case::profile(
        OverrideLayer::MapProfile,
        "busy~",
        Some("P/ini-overrides/profiles/busy~/Game.ini")
    )]
    #[case::index(
        OverrideLayer::MapIndex,
        "2",
        Some("P/ini-overrides/indices/2/Game.ini")
    )]
    #[case::index_wildcard(OverrideLayer::MapIndex, "2~", None)]
    #[case::unsafe_slot(OverrideLayer::Map, "../escape", None)]
    fn override_path_validates_slot(
        #[case] layer: OverrideLayer,
        #[case] slot: &str,
        #[case] expected: Option<&str>,
    ) {
        assert_eq!(
            build_override_path("P", "Game.ini", &LayerSelection::new(layer, slot)).as_deref(),
            expected
        );
    }

    #[rstest]
    #[case::bare("Game.ini", Some("Game.ini"))]
    #[case::unix_path("/Saved/WindowsServer/Game.ini", Some("Game.ini"))]
    #[case::windows_path(r"C:\Saved\WindowsServer\Game.ini", Some("Game.ini"))]
    #[case::mixed_separators(r"C:\Saved/WindowsServer\Game.ini", Some("Game.ini"))]
    #[case::extension_case("Game.INI", Some("Game.INI"))]
    #[case::wrong_extension("Game.yaml", None)]
    #[case::device("COM¹.ini", None)]
    #[case::empty("", None)]
    #[case::directory("Saved/", None)]
    fn config_filename_extracts_valid_leaf(#[case] filename: &str, #[case] expected: Option<&str>) {
        assert_eq!(config_filename(filename), expected);
    }

    #[rstest]
    #[case::parent_path("../Game.ini")]
    #[case::absolute_path(r"C:\Game.ini")]
    #[case::wrong_extension("Game.yaml")]
    #[case::device("COM¹.ini")]
    fn override_path_rejects_invalid_filenames(#[case] filename: &str) {
        assert!(build_override_path(
            "C:/P",
            filename,
            &LayerSelection::new(OverrideLayer::Map, "Quarry")
        )
        .is_none());
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

    #[rstest]
    #[case::missing(None)]
    #[case::invalid(Some("../escape"))]
    fn missing_or_invalid_profile_keeps_index_layer(#[case] profile: Option<&str>) {
        let context = MapContext::new("Quarry", Some(2));
        assert_eq!(
            select_layers(&context, profile),
            vec![
                LayerSelection::new(OverrideLayer::Map, "Quarry"),
                LayerSelection::new(OverrideLayer::MapIndex, "2"),
            ]
        );
    }

    #[test]
    fn profile_without_index_is_not_selected() {
        assert_eq!(
            select_layers(&MapContext::new("Quarry", None), Some("busy")),
            vec![LayerSelection::new(OverrideLayer::Map, "Quarry")]
        );
    }

    #[rstest]
    #[case::exact("FFA_Courtyard", &["~", "F~", "FFA_~", "~yard", "FFA~yard", "FFA_Courtyard"])]
    #[case::fallback("ffa_other", &["~", "F~", "FFA_~", "ffa_other"])]
    #[case::empty_wildcard("FFA_", &["~", "F~", "FFA_~", "FFA_"])]
    fn matching_slots_orders_by_specificity_then_name_and_deduplicates_case(
        #[values(OverrideLayer::Map, OverrideLayer::MapProfile)] layer: OverrideLayer,
        #[case] target: &str,
        #[case] expected: &[&str],
    ) {
        let folders = [
            "FFA_Courtyard",
            "ffa_courtyard",
            "FFA_~",
            "ffa_~",
            "F~",
            "~",
            "TDM_~",
            "~yard",
            "FFA~yard",
            "ffa~YARD",
        ]
        .map(String::from);
        assert_eq!(matching_slots(layer, target, &folders), expected);
    }

    #[rstest]
    #[case::suffix("~Courtyard", "FFA_Courtyard", true)]
    #[case::anchored_suffix("~Courtyard", "FFA_Courtyard_extra", false)]
    #[case::middle("FFA~yard", "FFA_Courtyard", true)]
    #[case::anchored_prefix("FFA~yard", "TDM_Courtyard", false)]
    #[case::multiple("~FFA~Court~yard~", "FFA_Courtyard", true)]
    #[case::adjacent_wildcards("FFA~~Courtyard", "FFA_Courtyard", true)]
    #[case::overlapping_ends("F~F", "F", false)]
    #[case::overlapping_literals("~ab~bc", "abc", false)]
    #[case::separate_literals("~ab~bc", "ababc", true)]
    #[case::out_of_order("~Court~FFA~", "FFA_Courtyard", false)]
    #[case::only_wildcards("~~~", "FFA_Courtyard", true)]
    #[case::unicode_empty("夜~間", "夜間", true)]
    #[case::unicode_middle("夜~間", "夜の時間", true)]
    fn wildcard_matching_is_anchored_and_literals_do_not_overlap(
        #[case] pattern: &str,
        #[case] target: &str,
        #[case] expected: bool,
    ) {
        assert_eq!(wildcard_match(pattern, target), expected);
    }

    #[test]
    fn indices_match_exactly() {
        assert_eq!(
            matching_slots(
                OverrideLayer::MapIndex,
                "2",
                &["~".into(), "2~".into(), "2".into()]
            ),
            ["2"]
        );
    }

    #[rstest]
    fn matching_slots_rejects_unsafe_target(
        #[values(OverrideLayer::Map, OverrideLayer::MapProfile, OverrideLayer::MapIndex)]
        layer: OverrideLayer,
    ) {
        assert!(matching_slots(layer, "../bad", &[]).is_empty());
    }

    #[rstest]
    #[case::parent("../~")]
    #[case::slash("foo/~")]
    #[case::backslash("foo\\~")]
    #[case::asterisk("foo*")]
    #[case::device("CON")]
    #[case::trailing_dot("foo~.")]
    #[case::query("foo?~")]
    fn rejects_unsafe_slots(
        #[values(OverrideLayer::Map, OverrideLayer::MapProfile)] layer: OverrideLayer,
        #[case] slot: &str,
    ) {
        assert!(!valid_slot(layer, slot));
    }

    #[rstest]
    fn only_maps_and_profiles_allow_wildcards(
        #[values(OverrideLayer::Map, OverrideLayer::MapProfile, OverrideLayer::MapIndex)]
        layer: OverrideLayer,
        #[values("~yard", "FFA~yard", "~FFA~yard~", "foo~~", "CON~")] slot: &str,
    ) {
        assert_eq!(valid_slot(layer, slot), layer != OverrideLayer::MapIndex);
    }

    #[rstest]
    fn filesystem_expansion_preserves_layer_precedence(
        #[values("Engine.ini", "Game.ini")] config: &str,
    ) {
        let root = std::env::temp_dir().join(format!(
            "unchained-wildcards-{}-{config}",
            std::process::id()
        ));
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
        let paths = expanded
            .iter()
            .filter_map(|selection| build_override_path(root.to_str().unwrap(), config, selection))
            .collect::<Vec<_>>();
        let expected = expanded
            .iter()
            .map(|selection| {
                format!(
                    "{}/ini-overrides/{}/{}/{config}",
                    root.display().to_string().replace('\\', "/"),
                    selection.layer.dir(),
                    selection.slot
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(paths, expected);
        std::fs::remove_dir_all(root).unwrap();
    }
}
