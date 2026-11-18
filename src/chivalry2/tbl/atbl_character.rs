//! ArgonSDK main: [ATBLCharacter](https://github.com/Chiv2-Community/ArgonSDK/blob/main/Source/TBL/Public/TBLCharacter.h).

use crate::events::combat::{CombatActor, DamageSource};
use crate::events::models::{Damage, GameEvent, Kill};
use crate::features::events::EVENT_SYSTEM;
use crate::chivalry2::tbl::{FDamageTakenEvent, FDeathDamageTakenEvent};
mod layout;
pub use layout::*;

define_pattern_resolver!(
    ATBLCharacter__OnKilled,
    ["4C 8B DC 55 41 55 41 57 49 8D AB ?? ?? ?? ?? 48 81 EC 30 03 00 00"]
);
#[unchained_macros::runtime_bindings]
impl ATBLCharacter {
    #[bind_resolved_func_detour(ATBLCharacter__OnKilled)]
    pub(crate) unsafe fn on_killed_detour(
        self: *mut Self,
        damage_event: *const FDeathDamageTakenEvent,
    ) {
        let this_ptr = self;
        if this_ptr.is_null() || damage_event.is_null() {
            unsafe {
                self.on_killed(damage_event);
            }
            return;
        }
        let death_event = unsafe { &*damage_event };
        let damage_taken = &death_event.damage_taken;

        // Native callbacks provide live events and objects on the game thread.
        let (victim, killers, source) = unsafe {
            (
                CombatActor::victim_from_damage_event(damage_taken)
                    .unwrap_or_else(|| CombatActor::new("UnknownVictim".into(), false)),
                CombatActor::killers_from_death_event(death_event),
                DamageSource::from_damage_event(damage_taken),
            )
        };
        let killer = killers
            .first()
            .cloned()
            .unwrap_or_else(|| CombatActor::new("UnknownKiller".into(), false));

        let kill_reason = death_event.kill_reason.as_str().to_string();

        strace!(
            "Kill: {} -> {} [{}] via {} ({:.2} dmg)",
            killer.name,
            victim.name,
            kill_reason,
            source.source,
            source.amount
        );

        EVENT_SYSTEM
            .game_event_publisher
            .publish(GameEvent::KillEvent(Kill {
                killer: killer.clone(),
                victim: victim.clone(),
                killers: if killers.is_empty() {
                    vec![killer.clone()]
                } else {
                    killers
                },
                kill_reason,
                random_seed: death_event.random_seed,
                dead_character_id: death_event.dead_character_id,
                attach_to_projectile: death_event.b_attach_to_projectile,
                source: Damage {
                    attacker: killer.name.clone(),
                    victim: victim.name.clone(),
                    attacker_actor: killer,
                    victim_actor: victim,
                    damage: source,
                },
            }));

        unsafe {
            self.on_killed(damage_event);
        }
    }
}

define_pattern_resolver!(
    ATBLCharacter__OnDamageTaken,
    ["4C 8B DC 55 57 41 55 41 56 49 8D AB ?? ?? ?? ?? 48 81 EC 58 02 00 00"]
);
#[unchained_macros::runtime_bindings]
impl ATBLCharacter {
    #[bind_resolved_func_detour(ATBLCharacter__OnDamageTaken)]
    pub(crate) unsafe fn on_damage_taken_detour(
        self: *mut Self,
        damage_event: *const FDamageTakenEvent,
    ) {
        let this_ptr = self;
        if this_ptr.is_null() || damage_event.is_null() {
            unsafe {
                self.on_damage_taken(damage_event);
            }
            return;
        }

        let damage_taken = unsafe { &*damage_event };
        // Native callbacks provide live events and objects on the game thread.
        let (attacker, victim, source) = unsafe {
            (
                CombatActor::killer_from_damage_event(damage_taken)
                    .unwrap_or_else(|| CombatActor::new("UnknownAttacker".into(), false)),
                CombatActor::victim_from_damage_event(damage_taken)
                    .or_else(|| CombatActor::from_actor(this_ptr.cast()))
                    .unwrap_or_else(|| CombatActor::new("UnknownVictim".into(), false)),
                DamageSource::from_damage_event(damage_taken),
            )
        };

        strace!(
            "Damage: {} -> {} via {} ({:.2} dmg, type={})",
            attacker.name,
            victim.name,
            source.source,
            source.amount,
            source.attack_type
        );

        EVENT_SYSTEM
            .game_event_publisher
            .publish(GameEvent::DamageEvent(Damage {
                attacker: attacker.name.clone(),
                victim: victim.name.clone(),
                attacker_actor: attacker,
                victim_actor: victim,
                damage: source,
            }));

        unsafe {
            self.on_damage_taken(damage_event);
        }
    }
}
