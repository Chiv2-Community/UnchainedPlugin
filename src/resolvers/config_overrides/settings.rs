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

    macro_rules! fixture {
        ($name:literal) => {
            include_str!(concat!(
                "../../../test-resources/match-rotation/",
                $name,
                ".yaml"
            ))
        };
    }

    #[test]
    fn array_positions_select_profiles_and_metadata_is_preserved() {
        let settings = parse_server_settings(fixture!("metadata-and-profiles")).unwrap();
        assert_eq!(settings.profile_at(0), Some("quiet"));
        assert_eq!(settings.profile_at(1), Some("busy"));
        assert_eq!(settings.profile_at(-1), None);
        assert_eq!(settings.profile_at(2), None);
        let entry = &settings.match_rotation[0];
        assert_eq!(entry.name, "First match");
        assert_eq!(entry.description, "Display only");
        assert_eq!(entry.map, "DisplayMap");
        assert_eq!(entry.mod_actors, ["ActorA", "ActorB"]);
        assert!(settings.match_rotation[1].mod_actors.is_empty());
    }

    #[test]
    fn rejects_duplicate_unsafe_or_missing_profiles() {
        for yaml in [
            fixture!("duplicate-profiles"),
            fixture!("unsafe-profile"),
            fixture!("empty-profile"),
            fixture!("missing-profile"),
            fixture!("invalid-mod-actors"),
            fixture!("legacy-profile-mapping"),
        ] {
            assert!(parse_server_settings(yaml).is_err(), "{yaml}");
        }
    }

    #[test]
    fn empty_rotation_is_valid() {
        for yaml in [
            fixture!("empty-document"),
            fixture!("empty-settings"),
            fixture!("empty-rotation"),
        ] {
            assert!(parse_server_settings(yaml)
                .unwrap()
                .match_rotation
                .is_empty());
        }
    }
}
