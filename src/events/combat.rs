//! Combat event snapshots. Resolve native objects on the game thread.

use crate::chivalry2::engine::{ACharacter, AController, APlayerState};
use crate::chivalry2::tbl::{ATBLCharacter, FDamageTakenEvent, FDeathDamageTakenEvent};
use crate::chivalry2::engine::UObject;
use std::fmt;

/// Combat participant's display name and bot status.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CombatActor {
    pub name: String,
    /// PlayerState flag, or a name/class-name heuristic when unavailable.
    pub is_bot: bool,
}

/// Damage amount, source display name, and event classification.
#[derive(Debug, Clone)]
pub struct DamageSource {
    pub amount: f32,
    /// Item, projectile, or damage-source name; otherwise `UnknownDamageSource`.
    pub source: String,
    /// `Suicide`, `Backstab`, `KillVolume`, or `Damage`, in precedence order.
    pub attack_type: &'static str,
}

impl fmt::Display for CombatActor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_bot {
            f.write_str("(bot) ")?;
        }
        f.write_str(&self.name)
    }
}

impl CombatActor {
    pub fn new(name: String, is_bot: bool) -> Self {
        Self { name, is_bot }
    }

    unsafe fn from_object(object: &UObject) -> Option<Self> {
        let cls = unsafe { object.class_name() }?;
        let name = object.display_name();
        let is_bot = cls.to_lowercase().contains("bot") || name.to_lowercase().contains("bot");
        Some(Self::new(name, is_bot))
    }

    unsafe fn from_player_state_view(ps: &APlayerState) -> Option<Self> {
        let name = ps.player_name_private.copy_to_string().ok()?;
        (!name.trim().is_empty()).then(|| Self::new(name.trim().to_string(), ps.is_bot()))
    }

    /// Resolve PlayerState identity, falling back to the UObject display name.
    ///
    /// # Safety
    /// Call on the game thread after globals initialization. Registered objects,
    /// relationship pointers and class metadata must remain valid for the lookup;
    /// native layouts must match the supported build. Liveness checks do not pin objects.
    /// Null inputs require no engine state.
    pub unsafe fn from_actor(ptr: *mut UObject) -> Option<Self> {
        if ptr.is_null() {
            return None;
        }
        unsafe {
            let Some(object) = UObject::from_ptr(ptr) else {
                swarn!("ignoring combat actor {:p}: not a live object", ptr);
                return None;
            };
            let actor = if let Some(character) = object.uobj_cast::<ACharacter>() {
                let pawn = &character.base_pawn;
                pawn.player_state()
                    .and_then(|ps| Self::from_player_state_view(ps))
                    .or_else(|| {
                        pawn.controller()?
                            .player_state()
                            .and_then(|ps| Self::from_player_state_view(ps))
                    })
                    .or_else(|| {
                        object
                            .uobj_cast::<ATBLCharacter>()?
                            .last_player_state()
                            .and_then(|ps| Self::from_player_state_view(ps))
                    })
            } else if let Some(controller) = object.uobj_cast::<AController>() {
                controller
                    .player_state()
                    .and_then(|ps| Self::from_player_state_view(ps))
            } else {
                object
                    .uobj_cast::<APlayerState>()
                    .and_then(|ps| Self::from_player_state_view(ps))
            };
            actor.or_else(|| Self::from_object(object))
        }
    }

    /// # Safety
    /// The event must be live; from_actor's contract applies to its native pointers.
    pub unsafe fn killer_from_damage_event(event: &FDamageTakenEvent) -> Option<Self> {
        unsafe {
            Self::from_actor(event.damage_instigator.cast())
                .or_else(|| Self::from_actor(event.damage_causer.cast()))
        }
    }

    /// # Safety
    /// The event and killers array must be live and readable; from_actor's contract
    /// applies to their native pointers.
    pub unsafe fn killers_from_death_event(event: &FDeathDamageTakenEvent) -> Vec<Self> {
        unsafe {
            let mut resolved: Vec<_> = event
                .killers
                .as_slice()
                .iter()
                .filter_map(|&ptr| Self::from_actor(ptr.cast()))
                .collect();
            if resolved.is_empty() {
                if let Some(actor) = Self::killer_from_damage_event(&event.damage_taken) {
                    resolved.push(actor);
                }
            }
            resolved
        }
    }

    /// # Safety
    /// The event must be live; from_actor's contract applies to its native pointers.
    pub unsafe fn victim_from_damage_event(event: &FDamageTakenEvent) -> Option<Self> {
        unsafe { Self::from_actor(event.damage_taker.cast()) }
    }
}

impl DamageSource {
    /// # Safety
    /// The event must be live; CombatActor::from_actor's engine-thread, lifetime
    /// and layout requirements apply to its native source pointers.
    pub unsafe fn from_damage_event(event: &FDamageTakenEvent) -> Self {
        Self {
            amount: event.damage,
            source: unsafe { Self::name_from_damage_event(event) },
            attack_type: Self::attack_type_from_damage_event(event),
        }
    }

    unsafe fn name_from_damage_event(event: &FDamageTakenEvent) -> String {
        // Prefer concrete weapon/projectile identity over abstract UDamageSource.
        [
            ("inventory_item", event.inventory_item.cast::<UObject>()),
            ("projectile", event.projectile.cast::<UObject>()),
            ("damage_source", event.damage_source.cast::<UObject>()),
        ]
        .into_iter()
        .find_map(|(kind, ptr)| {
            let name = unsafe { UObject::from_ptr(ptr) }?.display_name();
            if name.trim().is_empty() {
                return None;
            }
            if kind != "damage_source" {
                strace!("Kill source resolved via {}='{}'", kind, name);
            }
            Some(name)
        })
        .unwrap_or_else(|| "UnknownDamageSource".to_string())
    }

    fn attack_type_from_damage_event(event: &FDamageTakenEvent) -> &'static str {
        if event.b_suicide {
            "Suicide"
        } else if event.b_back_stab {
            "Backstab"
        } else if event.b_entered_kill_volume {
            "KillVolume"
        } else {
            "Damage"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case::test_case(false, "Knight"; "human")]
    #[test_case::test_case(true, "(bot) Knight"; "bot")]
    fn combat_actor_display_marks_bots(is_bot: bool, expected: &str) {
        assert_eq!(
            CombatActor::new("Knight".into(), is_bot).to_string(),
            expected
        );
    }

    #[test]
    fn null_actor_inputs_have_no_identity() {
        // Null inputs return before accessing engine globals or native objects.
        unsafe {
            assert_eq!(CombatActor::from_actor(std::ptr::null_mut()), None);
        }
    }

    #[test_case::test_case(false, false, false, "Damage"; "plain damage")]
    #[test_case::test_case(false, false, true, "KillVolume"; "kill volume")]
    #[test_case::test_case(false, true, false, "Backstab"; "backstab")]
    #[test_case::test_case(false, true, true, "Backstab"; "backstab beats kill volume")]
    #[test_case::test_case(true, false, false, "Suicide"; "suicide")]
    #[test_case::test_case(true, false, true, "Suicide"; "suicide beats kill volume")]
    #[test_case::test_case(true, true, false, "Suicide"; "suicide beats backstab")]
    #[test_case::test_case(true, true, true, "Suicide"; "suicide beats both")]
    fn missing_actors_and_sources_preserve_damage_and_attack_priority(
        suicide: bool,
        backstab: bool,
        kill_volume: bool,
        expected: &str,
    ) {
        // All fields permit zero: null pointers, numeric/name fields, false flags,
        // and EKillReason::Damage. No engine objects are dereferenced.
        let mut event: FDamageTakenEvent = unsafe { std::mem::zeroed() };
        event.damage = 42.5;
        event.b_suicide = suicide;
        event.b_back_stab = backstab;
        event.b_entered_kill_volume = kill_volume;
        unsafe {
            assert_eq!(CombatActor::killer_from_damage_event(&event), None);
            assert_eq!(CombatActor::victim_from_damage_event(&event), None);
        }

        let source = unsafe { DamageSource::from_damage_event(&event) };
        assert_eq!(source.amount, event.damage);
        assert_eq!(source.source, "UnknownDamageSource");
        assert_eq!(source.attack_type, expected);
    }
}
