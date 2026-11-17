//! Combat event snapshots. Resolve native objects on the game thread.

use crate::game::chivalry2::{
    ACharacter, AController, APlayerState, ATBLCharacter, FDamageTakenEvent,
    FDeathDamageTakenEvent, PlayerFlags,
};
use crate::ue::{UObject, UObjectType};
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
    pub attack_type: String,
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

    /// Use the UObject display name and bot-name heuristic.
    pub fn from_uobject(ptr: *mut UObject) -> Option<Self> {
        let object = unsafe { UObject::from_ptr(ptr) }?;
        let cls = unsafe { object.class_name() }?;
        let name = object.display_name();
        let is_bot = cls.to_lowercase().contains("bot") || name.to_lowercase().contains("bot");
        Some(Self::new(name, is_bot))
    }

    /// `via` identifies the source field when an invalid PlayerState is logged.
    pub fn from_player_state(player_state: *mut APlayerState, via: &str) -> Option<Self> {
        if player_state.is_null() {
            return None;
        }
        let Some(ps) = (unsafe { APlayerState::from_ptr(player_state) }) else {
            swarn!(
                "ignoring PlayerState {:p} from {}: not a live PlayerState",
                player_state,
                via
            );
            return None;
        };
        let name = ps.player_name_private.copy_to_string().ok()?;
        (!name.trim().is_empty()).then(|| {
            Self::new(
                name.trim().to_string(),
                ps.player_flags.contains(PlayerFlags::IS_A_BOT),
            )
        })
    }

    /// Resolve PlayerState identity, falling back to the UObject display name.
    pub fn from_actor(ptr: *mut UObject) -> Option<Self> {
        if ptr.is_null() {
            return None;
        }
        let Some(object) = (unsafe { UObject::from_ptr(ptr) }) else {
            swarn!("ignoring combat actor {:p}: not a live object", ptr);
            return None;
        };
        if let Some(character) = unsafe { object.uobj_cast::<ACharacter>() } {
            let pawn = &character.base_pawn;
            return Self::from_player_state(
                pawn.player_state as *mut APlayerState,
                "Pawn.PlayerState",
            )
            .or_else(|| {
                let controller = unsafe { AController::from_ptr(pawn.controller) }?;
                Self::from_player_state(controller.player_state, "Pawn.Controller.PlayerState")
            })
            .or_else(|| {
                let character = unsafe { object.uobj_cast::<ATBLCharacter>() }?;
                Self::from_player_state(
                    character.last_player_state as *mut APlayerState,
                    "TBLCharacter.LastPlayerState",
                )
            })
            .or_else(|| Self::from_uobject(ptr));
        }
        if let Some(controller) = unsafe { object.uobj_cast::<AController>() } {
            return Self::from_player_state(controller.player_state, "Controller.PlayerState")
                .or_else(|| Self::from_uobject(ptr));
        }
        if unsafe { object.uobj_cast::<APlayerState>() }.is_some() {
            return Self::from_player_state(ptr.cast(), "PlayerState")
                .or_else(|| Self::from_uobject(ptr));
        }
        Self::from_uobject(ptr)
    }

    pub fn killer_from_damage_event(event: &FDamageTakenEvent) -> Option<Self> {
        Self::from_actor(event.damage_instigator.cast())
            .or_else(|| Self::from_actor(event.damage_causer.cast()))
    }

    pub fn killers_from_death_event(event: &FDeathDamageTakenEvent) -> Vec<Self> {
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

    pub fn victim_from_damage_event(event: &FDamageTakenEvent) -> Option<Self> {
        Self::from_actor(event.damage_taker.cast())
    }
}

impl DamageSource {
    pub fn from_damage_event(event: &FDamageTakenEvent) -> Self {
        Self {
            amount: event.damage,
            source: Self::name_from_damage_event(event),
            attack_type: Self::attack_type_from_damage_event(event),
        }
    }

    fn name_from_damage_event(event: &FDamageTakenEvent) -> String {
        // Prefer concrete weapon/projectile identity over abstract UDamageSource.
        let candidates = [
            ("inventory_item", event.inventory_item.cast::<UObject>()),
            ("projectile", event.projectile.cast::<UObject>()),
            ("damage_source", event.damage_source.cast::<UObject>()),
        ];
        for (kind, ptr) in candidates {
            if ptr.is_null() {
                continue;
            }
            if let Some(object) = unsafe { UObject::from_ptr(ptr) } {
                let name = object.display_name();
                if name.trim().is_empty() {
                    continue;
                }
                if kind != "damage_source" {
                    strace!("Kill source resolved via {}='{}'", kind, name);
                }
                return name;
            }
        }
        "UnknownDamageSource".to_string()
    }

    fn attack_type_from_damage_event(event: &FDamageTakenEvent) -> String {
        if event.b_suicide {
            return "Suicide".into();
        }
        if event.b_back_stab {
            return "Backstab".into();
        }
        if event.b_entered_kill_volume {
            return "KillVolume".into();
        }
        "Damage".into()
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
        assert_eq!(CombatActor::from_uobject(std::ptr::null_mut()), None);
        assert_eq!(
            CombatActor::from_player_state(std::ptr::null_mut(), "test"),
            None
        );
        assert_eq!(CombatActor::from_actor(std::ptr::null_mut()), None);
    }

    #[test]
    fn missing_actors_and_sources_preserve_damage_and_attack_priority() {
        // All fields permit zero: null pointers, numeric/name fields, false flags,
        // and EKillReason::Damage. No engine objects are dereferenced.
        let mut event: FDamageTakenEvent = unsafe { std::mem::zeroed() };
        event.damage = 42.5;
        assert_eq!(CombatActor::killer_from_damage_event(&event), None);
        assert_eq!(CombatActor::victim_from_damage_event(&event), None);

        for (suicide, backstab, kill_volume, expected) in [
            (false, false, false, "Damage"),
            (false, false, true, "KillVolume"),
            (false, true, true, "Backstab"),
            (true, true, true, "Suicide"),
        ] {
            event.b_suicide = suicide;
            event.b_back_stab = backstab;
            event.b_entered_kill_volume = kill_volume;
            let source = DamageSource::from_damage_event(&event);
            assert_eq!(source.amount, event.damage);
            assert_eq!(source.source, "UnknownDamageSource");
            assert_eq!(source.attack_type, expected);
        }
    }
}
