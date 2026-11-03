use super::selection::{map_folder, MapContext};

#[derive(Clone, Default)]
pub(crate) struct Destination {
    // Cache invalidation version, not the map rotation index.
    pub generation: u64,
    pub selection: MapContext,
    pub world: Option<usize>,
}

#[derive(Default)]
pub(super) struct TravelState {
    pub current: Destination,
    pending_rotation: Option<MapContext>,
    startup_finished: bool,
}

impl TravelState {
    pub fn can_capture_startup(&self, map: &str, starting_map: Option<&str>) -> bool {
        !self.startup_finished
            && starting_map
                .and_then(map_folder)
                .zip(map_folder(map))
                .is_some_and(|(starting, destination)| destination.eq_ignore_ascii_case(&starting))
    }

    pub fn capture_startup(&mut self, map: &str, starting_map: Option<&str>, index: Option<i32>) {
        if !self.can_capture_startup(map, starting_map) {
            return;
        }
        self.startup_finished = true;
        if let Some(index) = index.filter(|index| *index >= 0) {
            self.pending_rotation = Some(MapContext::new(map, Some(index)));
        }
    }

    pub fn accept_rotation(&mut self, pending: Option<MapContext>) {
        // Manual/unmatched travel also replaces the previous pending destination.
        self.startup_finished = true;
        self.pending_rotation = pending;
    }
    /// LoadMap and GameMode creation for the same load share one selection/version.
    /// Return the previous destination so a failed LoadMap can restore it.
    pub fn enter(&mut self, map: &str, world: Option<usize>) -> Destination {
        let context = &mut self.current;
        let pending = &mut self.pending_rotation;
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

    pub fn restore(&mut self, previous: Destination) {
        let context = &mut self.current;
        let generation = context.generation.wrapping_add(1);
        *context = Destination {
            generation,
            ..previous
        };
        self.pending_rotation = None;
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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn startup_uses_explicit_slot_once_and_ignores_frontend() {
        let mut travel = TravelState::default();
        travel.capture_startup("Frontend", Some("Same"), Some(2));
        assert!(travel.pending_rotation.is_none());
        travel.capture_startup("/Game/Maps/Same?listen", Some("Same"), Some(2));
        assert_eq!(travel.pending_rotation.as_ref().unwrap().index, Some(2));
        travel.enter("Same", None);
        travel.enter("Same", Some(42));
        assert_eq!(travel.current.selection.index, Some(2));
        travel.capture_startup("Same", Some("Same"), Some(0));
        assert!(travel.pending_rotation.is_none());
    }

    #[rstest]
    #[case::missing(None)]
    #[case::negative(Some(-1))]
    fn startup_rejects_invalid_index_and_consumes_attempt(#[case] index: Option<i32>) {
        let mut travel = TravelState::default();
        travel.capture_startup("Same", Some("Same"), index);
        assert!(travel.pending_rotation.is_none());
        travel.capture_startup("Same", Some("Same"), Some(0));
        assert!(travel.pending_rotation.is_none());
    }

    #[test]
    fn startup_requires_initial_map_but_not_yaml_profile() {
        let mut travel = TravelState::default();
        travel.capture_startup("Same", None, Some(0));
        assert!(travel.pending_rotation.is_none());
        // An index needs no YAML profile: its exact index directory still applies.
        travel.capture_startup("Same", Some("Same"), Some(99));
        assert_eq!(travel.pending_rotation.unwrap().index, Some(99));
    }

    #[test]
    fn accepted_travel_prevents_late_startup_reselection() {
        let mut travel = TravelState::default();
        travel.accept_rotation(None);
        assert!(!travel.can_capture_startup("Same", Some("Same")));
        travel.capture_startup("Same", Some("Same"), Some(0));
        assert!(travel.pending_rotation.is_none());
    }

    #[rstest]
    #[case::manual(None)]
    #[case::next_slot(Some(1))]
    fn accepted_travel_replaces_pending_destination(#[case] index: Option<i32>) {
        let mut travel = TravelState::default();
        travel.accept_rotation(Some(MapContext::new("A", Some(0))));
        let pending = index.map(|index| MapContext::new("B", Some(index)));
        travel.accept_rotation(pending.clone());
        assert_eq!(travel.pending_rotation, pending);
    }

    #[test]
    fn successive_native_slots_survive_new_generations_and_wrap() {
        let maps = ["Same", "Other", "Same"];
        let mut travel = TravelState::default();
        travel.capture_startup("Same", Some("Same"), Some(0));
        travel.enter("Same", Some(1));
        let mut baseline_index = 0;
        for (world, live_index) in [(2, 1), (3, 2), (4, 0)] {
            let destination = maps[live_index as usize];
            let pending = capture_rotation(destination, live_index, Some(destination), |index| {
                baseline_index = index;
            });
            travel.accept_rotation(pending);
            let previous_generation = travel.current.generation;
            travel.enter(destination, Some(world));
            assert!(travel.current.generation > previous_generation);
            assert_eq!(travel.current.selection.index, Some(baseline_index));
            assert_eq!(baseline_index, live_index);
        }
    }

    #[rstest]
    #[case::different_map("Manual", 1, Some("Other"))]
    #[case::negative_index("Same", -1, Some("Same"))]
    #[case::missing_entry("Same", 9, None)]
    #[case::different_package("/Game/A/Same", 0, Some("/Game/B/Same"))]
    fn invalid_rotation_never_writes_baseline_index(
        #[case] url: &str,
        #[case] index: i32,
        #[case] entry: Option<&str>,
    ) {
        assert!(capture_rotation(url, index, entry, |_| {
            panic!("invalid rotation must not write baseline")
        })
        .is_none());
    }

    #[rstest]
    #[case::duplicate_map("Same?NumBots=8", 2, Some(2))]
    #[case::different_map("Different", 2, None)]
    #[case::past_end("Same", 5, None)]
    #[case::negative("Same", -1, None)]
    fn validates_the_live_slot_instead_of_searching_for_a_matching_map(
        #[case] url: &str,
        #[case] index: i32,
        #[case] expected: Option<i32>,
    ) {
        let entries = ["Same", "Other", "Same"];
        let entry = usize::try_from(index)
            .ok()
            .and_then(|index| entries.get(index))
            .copied();
        assert_eq!(
            rotation_context(url, index, entry).and_then(|context| context.index),
            expected
        );
    }

    #[test]
    fn qualified_rotation_map_ignores_ascii_case_and_url_options() {
        assert_eq!(
            rotation_context("/game/maps/SAME?x", 0, Some("/Game/Maps/Same"))
                .unwrap()
                .index,
            Some(0)
        );
    }

    #[test]
    fn ordinary_load_preserves_slot_through_game_mode_creation() {
        let mut travel = TravelState::default();
        travel.accept_rotation(rotation_context("B", 2, Some("B")));
        travel.enter("B", None);
        let generation = travel.current.generation;
        travel.enter("B", Some(42));
        assert_eq!(travel.current.selection.index, Some(2));
        assert_eq!(travel.current.generation, generation);
        assert!(travel.pending_rotation.is_none());
    }

    #[test]
    fn seamless_transition_keeps_final_destination_slot() {
        let mut travel = TravelState::default();
        travel.accept_rotation(rotation_context("Final", 3, Some("Final")));
        travel.enter("Transition", Some(1));
        assert_eq!(travel.current.selection.index, None);
        assert!(travel.pending_rotation.is_some());
        travel.enter("Final", Some(2));
        assert_eq!(travel.current.selection.index, Some(3));
        assert!(travel.pending_rotation.is_none());
    }

    #[test]
    fn new_world_on_same_map_does_not_inherit_a_slot() {
        let mut travel = TravelState::default();
        travel.accept_rotation(rotation_context("Same", 0, Some("Same")));
        travel.enter("Same", Some(1));
        let generation = travel.current.generation;
        travel.accept_rotation(rotation_context("Same", 2, Some("Same")));
        travel.enter("Same", Some(2));
        assert_eq!(travel.current.selection.index, Some(2));
        assert!(travel.current.generation > generation);
        travel.enter("Same", Some(3));
        assert_eq!(travel.current.selection.index, None);
    }

    #[test]
    fn manual_load_discards_mismatched_pending_rotation() {
        let mut travel = TravelState::default();
        travel.accept_rotation(rotation_context("B", 1, Some("B")));
        travel.enter("Manual", None);
        assert!(travel.pending_rotation.is_none());
        assert_eq!(travel.current.selection.index, None);
        travel.enter("B", Some(2));
        assert_eq!(travel.current.selection.index, None);
    }

    #[test]
    fn failed_load_restores_context_and_invalidates_overlays() {
        let mut travel = TravelState::default();
        travel.accept_rotation(rotation_context("A", 0, Some("A")));
        travel.enter("A", Some(1));
        travel.accept_rotation(rotation_context("B", 1, Some("B")));
        let previous = travel.enter("B", None);
        let generation = travel.current.generation;
        travel.restore(previous);
        assert_eq!(travel.current.selection.map.as_deref(), Some("A"));
        assert_eq!(travel.current.selection.index, Some(0));
        assert_eq!(travel.current.world, Some(1));
        assert!(travel.current.generation > generation);
        assert!(travel.pending_rotation.is_none());
    }
}
