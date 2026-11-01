use super::selection::{map_folder, MapContext};

#[derive(Clone, Default)]
pub(super) struct Context {
    pub generation: u64,
    pub selection: MapContext,
    pub world: Option<usize>,
}

#[derive(Default)]
pub(super) struct StartupSelection {
    attempted: bool,
}

impl StartupSelection {
    pub fn can_capture(&self, map: &str, starting_map: Option<&str>) -> bool {
        !self.attempted
            && starting_map
                .and_then(map_folder)
                .zip(map_folder(map))
                .is_some_and(|(starting, destination)| destination.eq_ignore_ascii_case(&starting))
    }

    /// Only the configured initial gameplay destination may consume the baseline slot.
    pub fn capture(
        &mut self,
        map: &str,
        starting_map: Option<&str>,
        index: Option<i32>,
    ) -> Option<MapContext> {
        if !self.can_capture(map, starting_map) {
            return None;
        }
        self.attempted = true;
        Some(MapContext::new(
            map,
            Some(index.filter(|index| *index >= 0)?),
        ))
    }

    pub fn finish(&mut self) {
        self.attempted = true;
    }
}

/// The live field is already advanced at ProcessServerTravel. Never infer a slot
/// by searching map names: a rotation may contain the same map more than once.
pub(super) fn rotation_context(url: &str, index: i32, entry: Option<&str>) -> Option<MapContext> {
    if index < 0 {
        return None;
    }
    let entry = entry?;
    let destination = map_folder(url)?;
    if !destination.eq_ignore_ascii_case(&map_folder(entry)?) {
        return None;
    }
    // If both names are qualified, matching only their leaves would be ambiguous.
    let package = |name: &str| {
        name.split('?')
            .next()
            .unwrap_or_default()
            .replace('\\', "/")
            .split('.')
            .next()
            .unwrap_or_default()
            .to_owned()
    };
    let url_package = package(url);
    let entry_package = package(entry);
    if url_package.contains('/')
        && entry_package.contains('/')
        && !url_package.eq_ignore_ascii_case(&entry_package)
    {
        return None;
    }
    Some(MapContext::new(url, Some(index)))
}

/// Preserve only a validated live slot before publishing its destination context.
pub(super) fn capture_rotation(
    url: &str,
    index: i32,
    entry: Option<&str>,
    preserve: impl FnOnce(i32),
) -> Option<MapContext> {
    let selection = rotation_context(url, index, entry)?;
    preserve(index);
    Some(selection)
}

pub(super) fn enter_context(
    context: &mut Context,
    pending: &mut Option<MapContext>,
    map: &str,
    world: Option<usize>,
) -> Context {
    let previous = context.clone();
    let mut selection = MapContext::new(map, None);
    let same_map = |a: &MapContext, b: &MapContext| {
        a.map
            .as_ref()
            .zip(b.map.as_ref())
            .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
    };
    let continuing_load = world.is_some()
        && same_map(&context.selection, &selection)
        && (context.world.is_none() || context.world == world);
    if pending
        .as_ref()
        .is_some_and(|pending| same_map(pending, &selection))
    {
        selection.index = pending.take().and_then(|pending| pending.index);
    } else {
        // An ordinary LoadMap supersedes a different pending destination. A seamless
        // transition world's GameMode must not consume the final destination's slot.
        if world.is_none() {
            *pending = None;
        }
        if continuing_load {
            selection.index = context.selection.index;
        }
    }
    if !continuing_load || context.selection.index != selection.index {
        context.generation = context.generation.wrapping_add(1);
    }
    context.selection = selection;
    context.world = world;
    previous
}

pub(super) fn restore_context(
    context: &mut Context,
    pending: &mut Option<MapContext>,
    previous: Context,
) {
    let generation = context.generation.wrapping_add(1);
    *context = Context {
        generation,
        ..previous
    };
    *pending = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_uses_explicit_slot_once_and_ignores_frontend() {
        let mut startup = StartupSelection::default();
        assert!(startup.capture("Frontend", Some("Same"), Some(2)).is_none());
        let mut pending = startup.capture("/Game/Maps/Same?listen", Some("Same"), Some(2));
        assert_eq!(pending.as_ref().unwrap().index, Some(2));
        let mut context = Context::default();
        enter_context(&mut context, &mut pending, "Same", None);
        enter_context(&mut context, &mut pending, "Same", Some(42));
        assert_eq!(context.selection.index, Some(2));
        assert!(startup.capture("Same", Some("Same"), Some(0)).is_none());
    }

    #[test]
    fn startup_requires_explicit_nonnegative_index_not_yaml_metadata() {
        for index in [None, Some(-1)] {
            let mut startup = StartupSelection::default();
            assert!(startup.capture("Same", Some("Same"), index).is_none());
            assert!(startup.capture("Same", Some("Same"), Some(0)).is_none());
        }
        let mut startup = StartupSelection::default();
        assert!(startup.capture("Same", None, Some(0)).is_none());
        // An index needs no YAML profile: its exact index directory still applies.
        assert_eq!(
            startup
                .capture("Same", Some("Same"), Some(99))
                .unwrap()
                .index,
            Some(99)
        );
    }

    #[test]
    fn accepted_travel_prevents_late_startup_reselection() {
        let mut startup = StartupSelection::default();
        startup.finish();
        assert!(startup.capture("Same", Some("Same"), Some(0)).is_none());
    }

    #[test]
    fn successive_native_slots_survive_new_generations_and_wrap() {
        let maps = ["Same", "Other", "Same"];
        let mut context = Context::default();
        let mut startup = StartupSelection::default();
        let mut pending = startup.capture("Same", Some("Same"), Some(0));
        enter_context(&mut context, &mut pending, "Same", Some(1));
        let mut baseline_index = 0;
        for (world, live_index) in [(2, 1), (3, 2), (4, 0)] {
            let destination = maps[live_index as usize];
            pending = capture_rotation(destination, live_index, Some(destination), |index| {
                baseline_index = index;
            });
            let previous_generation = context.generation;
            enter_context(&mut context, &mut pending, destination, Some(world));
            assert!(context.generation > previous_generation);
            assert_eq!(context.selection.index, Some(baseline_index));
            assert_eq!(baseline_index, live_index);
        }
    }

    #[test]
    fn invalid_rotation_never_writes_baseline_index() {
        for (url, index, entry) in [
            ("Manual", 1, Some("Other")),
            ("Same", -1, Some("Same")),
            ("Same", 9, None),
            ("/Game/A/Same", 0, Some("/Game/B/Same")),
        ] {
            assert!(capture_rotation(url, index, entry, |_| {
                panic!("invalid rotation must not write baseline")
            })
            .is_none());
        }
    }

    #[test]
    fn validates_the_live_slot_instead_of_searching_for_a_matching_map() {
        let entries = ["Same", "Other", "Same"];
        assert_eq!(
            rotation_context("Same?NumBots=8", 2, entries.get(2).copied())
                .unwrap()
                .index,
            Some(2)
        );
        assert!(rotation_context("Different", 2, entries.get(2).copied()).is_none());
        assert!(rotation_context("Same", 5, entries.get(5).copied()).is_none());
        assert!(rotation_context("Same", -1, Some("Same")).is_none());
        assert!(rotation_context("/Game/A/Same", 0, Some("/Game/B/Same")).is_none());
        assert_eq!(
            rotation_context("/game/maps/SAME?x", 0, Some("/Game/Maps/Same"))
                .unwrap()
                .index,
            Some(0)
        );
    }

    #[test]
    fn ordinary_load_preserves_slot_through_game_mode_creation() {
        let mut context = Context::default();
        let mut pending = rotation_context("B", 2, Some("B"));
        enter_context(&mut context, &mut pending, "B", None);
        let generation = context.generation;
        enter_context(&mut context, &mut pending, "B", Some(42));
        assert_eq!(context.selection.index, Some(2));
        assert_eq!(context.generation, generation);
        assert!(pending.is_none());
    }

    #[test]
    fn seamless_transition_keeps_final_destination_slot() {
        let mut context = Context::default();
        let mut pending = rotation_context("Final", 3, Some("Final"));
        enter_context(&mut context, &mut pending, "Transition", Some(1));
        assert_eq!(context.selection.index, None);
        assert!(pending.is_some());
        enter_context(&mut context, &mut pending, "Final", Some(2));
        assert_eq!(context.selection.index, Some(3));
        assert!(pending.is_none());
    }

    #[test]
    fn new_world_on_same_map_does_not_inherit_a_slot() {
        let mut context = Context::default();
        let mut pending = rotation_context("Same", 0, Some("Same"));
        enter_context(&mut context, &mut pending, "Same", Some(1));
        let generation = context.generation;
        pending = rotation_context("Same", 2, Some("Same"));
        enter_context(&mut context, &mut pending, "Same", Some(2));
        assert_eq!(context.selection.index, Some(2));
        assert!(context.generation > generation);
        enter_context(&mut context, &mut pending, "Same", Some(3));
        assert_eq!(context.selection.index, None);
    }

    #[test]
    fn manual_load_discards_mismatched_pending_rotation() {
        let mut context = Context::default();
        let mut pending = rotation_context("B", 1, Some("B"));
        enter_context(&mut context, &mut pending, "Manual", None);
        assert!(pending.is_none());
        assert_eq!(context.selection.index, None);
        enter_context(&mut context, &mut pending, "B", Some(2));
        assert_eq!(context.selection.index, None);
    }

    #[test]
    fn failed_load_restores_context_and_invalidates_overlays() {
        let mut context = Context::default();
        let mut pending = rotation_context("A", 0, Some("A"));
        enter_context(&mut context, &mut pending, "A", Some(1));
        pending = rotation_context("B", 1, Some("B"));
        let previous = enter_context(&mut context, &mut pending, "B", None);
        let generation = context.generation;
        restore_context(&mut context, &mut pending, previous);
        assert_eq!(context.selection.map.as_deref(), Some("A"));
        assert_eq!(context.selection.index, Some(0));
        assert_eq!(context.world, Some(1));
        assert!(context.generation > generation);
        assert!(pending.is_none());
    }
}
