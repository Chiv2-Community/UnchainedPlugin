use std::collections::HashSet;

use super::selection::valid_component;

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerSettings {
    #[serde(default, rename = "match-rotation")]
    pub match_rotation: Vec<MatchRotationEntry>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
#[allow(dead_code)] // Display metadata is parsed but has no runtime behavior yet.
pub struct MatchRotationEntry {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub profile: String,
    #[serde(default)]
    pub map: String,
    #[serde(default)]
    pub mod_actors: Vec<String>,
}

impl ServerSettings {
    pub fn profile_at(&self, index: i32) -> Option<&str> {
        self.match_rotation
            .get(usize::try_from(index).ok()?)
            .map(|entry| entry.profile.as_str())
    }
}

pub fn parse_server_settings(yaml: &str) -> Result<ServerSettings, serde_yaml::Error> {
    if yaml.trim().is_empty() {
        return Ok(ServerSettings::default());
    }
    let settings: ServerSettings = serde_yaml::from_str(yaml)?;
    let mut profiles = HashSet::new();
    for (index, entry) in settings.match_rotation.iter().enumerate() {
        if !valid_component(&entry.profile) {
            return Err(<serde_yaml::Error as serde::de::Error>::custom(format!(
                "match-rotation[{index}].profile must be a safe directory name"
            )));
        }
        if !profiles.insert(entry.profile.to_ascii_lowercase()) {
            return Err(<serde_yaml::Error as serde::de::Error>::custom(format!(
                "match-rotation[{index}].profile duplicates another profile: {}",
                entry.profile
            )));
        }
    }
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    macro_rules! fixture {
        ($name:literal) => {
            include_str!(concat!(
                "../../test-resources/match-rotation/",
                $name,
                ".yaml"
            ))
        };
    }

    #[test]
    fn metadata_is_preserved() {
        let settings = parse_server_settings(fixture!("metadata-and-profiles")).unwrap();
        let entry = &settings.match_rotation[0];
        assert_eq!(entry.name, "First match");
        assert_eq!(entry.description, "Display only");
        assert_eq!(entry.map, "DisplayMap");
        assert_eq!(entry.mod_actors, ["ActorA", "ActorB"]);
        assert!(settings.match_rotation[1].mod_actors.is_empty());
    }

    #[rstest]
    #[case::first(0, Some("quiet"))]
    #[case::second(1, Some("busy"))]
    #[case::negative(-1, None)]
    #[case::past_end(2, None)]
    fn array_positions_select_profiles(#[case] index: i32, #[case] expected: Option<&str>) {
        let settings = parse_server_settings(fixture!("metadata-and-profiles")).unwrap();
        assert_eq!(settings.profile_at(index), expected);
    }

    #[rstest]
    #[case::duplicate(fixture!("duplicate-profiles"))]
    #[case::unsafe_name(fixture!("unsafe-profile"))]
    #[case::empty_profile(fixture!("empty-profile"))]
    #[case::missing_profile(fixture!("missing-profile"))]
    #[case::invalid_mod_actors(fixture!("invalid-mod-actors"))]
    #[case::legacy_mapping(fixture!("legacy-profile-mapping"))]
    fn rejects_invalid_settings(#[case] yaml: &str) {
        assert!(parse_server_settings(yaml).is_err());
    }

    #[rstest]
    #[case::empty_document(fixture!("empty-document"))]
    #[case::empty_settings(fixture!("empty-settings"))]
    #[case::empty_rotation(fixture!("empty-rotation"))]
    fn empty_rotation_is_valid(#[case] yaml: &str) {
        assert!(parse_server_settings(yaml)
            .unwrap()
            .match_rotation
            .is_empty());
    }

    #[test]
    fn profile_metadata_matches_layer_fixture() {
        let root = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/test-resources/test-configs/config-overrides"
        ));
        let yaml = std::fs::read_to_string(root.join("unchained.yaml")).unwrap();
        let settings = parse_server_settings(&yaml).unwrap();
        let profile = settings.profile_at(1).unwrap();
        assert!(root
            .join("profiles")
            .join(profile)
            .join("Engine.ini")
            .is_file());
    }
}
