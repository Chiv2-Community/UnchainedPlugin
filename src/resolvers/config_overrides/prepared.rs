use std::collections::HashMap;

use super::selection::{
    build_override_paths, build_profile_override_path, config_filename, LayerSelection,
};

/// A destination's complete path selection, independent of native cache discovery.
#[derive(Default)]
pub(super) struct PreparedOverrides {
    paths: HashMap<String, Vec<String>>,
}

impl PreparedOverrides {
    pub fn new(root: &str, layers: &[LayerSelection]) -> Self {
        let mut prepared = Self::default();
        for selection in layers {
            let folder = build_profile_override_path(root, selection.layer, &selection.slot, "");
            let Ok(entries) = std::fs::read_dir(folder) else {
                continue;
            };
            let mut files = entries
                .flatten()
                .filter(|entry| entry.path().is_file())
                .filter_map(|entry| {
                    let name = entry.file_name().into_string().ok()?;
                    config_filename(&name)?;
                    Some(name)
                })
                .collect::<Vec<_>>();
            files.sort();
            files.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
            for name in files {
                prepared
                    .paths
                    .entry(name.to_ascii_lowercase())
                    .or_default()
                    .extend(build_override_paths(
                        root,
                        &name,
                        std::slice::from_ref(selection),
                    ));
            }
        }
        prepared
    }

    pub fn paths(&self, filename: &str) -> &[String] {
        config_filename(filename)
            .and_then(|name| self.paths.get(&name.to_ascii_lowercase()))
            .map_or(&[], Vec::as_slice)
    }

    pub fn categories(&self) -> impl Iterator<Item = String> + '_ {
        self.paths
            .keys()
            .map(|name| name[..name.len() - 4].to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::super::selection::{expand_layers, select_layers, MapContext};
    use super::*;

    #[test]
    fn destination_preparation_freezes_paths_for_late_config_discovery() {
        let root = std::env::temp_dir().join(format!("unchained-prepared-{}", std::process::id()));
        let root = root.to_str().unwrap();
        for (folder, config) in [
            ("maps/~", "Game.ini"),
            ("maps/A", "Game.ini"),
            ("profiles/busy", "Game.ini"),
            ("indices/2", "Game.ini"),
            ("indices/2", "ModConfig.ini"),
            ("maps/B", "Engine.ini"),
        ] {
            let folder = format!("{root}/ini-overrides/{folder}");
            std::fs::create_dir_all(&folder).unwrap();
            std::fs::write(format!("{folder}/{config}"), "[Test]\nValue=1").unwrap();
        }
        let prepare = |map, index, profile| {
            PreparedOverrides::new(
                root,
                &expand_layers(root, &select_layers(&MapContext::new(map, index), profile)),
            )
        };
        let outgoing = prepare("A", Some(2), Some("busy"));
        assert_eq!(
            outgoing.paths(r"C:\Saved\gAmE.INI"),
            [
                format!("{root}/ini-overrides/maps/~/Game.ini"),
                format!("{root}/ini-overrides/maps/A/Game.ini"),
                format!("{root}/ini-overrides/profiles/busy/Game.ini"),
                format!("{root}/ini-overrides/indices/2/Game.ini"),
            ]
        );
        let late_path = format!("{root}/ini-overrides/indices/2/ModConfig.ini");
        std::fs::remove_file(&late_path).unwrap();
        // A first read by a mod uses the frozen selection, not another directory scan.
        assert_eq!(outgoing.paths("ModConfig.ini"), [late_path]);
        assert!(outgoing.paths("Unknown.ini").is_empty());
        assert!(outgoing.paths("../CON.ini").is_empty());

        let destination = prepare("B", None, None);
        let mut refreshed = outgoing
            .categories()
            .collect::<std::collections::HashSet<_>>();
        refreshed.extend(destination.categories());
        assert_eq!(
            refreshed,
            ["game", "modconfig", "engine"].map(String::from).into()
        );
        assert_eq!(destination.paths("Game.ini").len(), 1);
        assert!(destination.paths("ModConfig.ini").is_empty());
        // Failed-load restoration selects A again, without retaining B's paths.
        let restored = prepare("A", Some(2), Some("busy"));
        assert_eq!(restored.paths("Game.ini"), outgoing.paths("Game.ini"));
        assert!(restored.paths("Engine.ini").is_empty());
        assert!(restored.paths("ModConfig.ini").is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
