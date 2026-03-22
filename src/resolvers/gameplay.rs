use std::os::raw::c_void;

use crate::events::models::{CombatActor, Damage, DamageSource, GameEvent, Kill};
use crate::features::events::EVENT_SYSTEM;
use crate::game::chivalry2::{AController, APlayerState, ATBLCharacter, FDeathDamageTakenEvent, FDamageTakenEvent, PlayerFlags};
use crate::ue::{UObject, UStruct};

define_pattern_resolver!(ATBLCharacter__OnKilled, [
    "4C 8B DC 55 41 55 41 57 49 8D AB ?? ?? ?? ?? 48 81 EC 30 03 00 00"
]);

fn object_class_name(object_ptr: *mut c_void) -> Option<String> {
    let object = unsafe { (object_ptr as *const UObject).as_ref() }?;
    let class_ptr = object.uobject_base_utility.uobject_base.class_private;
    let class = unsafe { class_ptr.as_ref() }?;
    Some(class.ustruct.ufield.uobject.uobject_base_utility.uobject_base.name_private.to_string())
}

fn object_name(object_ptr: *mut c_void) -> Option<String> {
    let object = unsafe { (object_ptr as *const UObject).as_ref() }?;
    Some(object.uobject_base_utility.uobject_base.name_private.to_string())
}

fn object_inherits_from(object_ptr: *mut c_void, needle: &str) -> bool {
    let object = match unsafe { (object_ptr as *const UObject).as_ref() } {
        Some(o) => o,
        None => return false,
    };

    let mut current = match unsafe { object.uobject_base_utility.uobject_base.class_private.as_ref() } {
        Some(class) => Some(&class.ustruct as *const UStruct),
        None => None,
    };

    while let Some(struct_ptr) = current {
        let ustruct = match unsafe { struct_ptr.as_ref() } {
            Some(s) => s,
            None => break,
        };

        let class_name = ustruct
            .ufield
            .uobject
            .uobject_base_utility
            .uobject_base
            .name_private
            .to_string();

        if class_name.contains(needle) {
            return true;
        }

        current = if ustruct.super_struct.is_null() {
            None
        } else {
            Some(ustruct.super_struct)
        };
    }

    false
}

#[derive(Debug, Clone)]
struct ResolvedActor {
    name: String,
    is_bot: bool,
}

fn trim_instance_suffix(name: &str) -> String {
    match name.rsplit_once('_') {
        Some((base, suffix)) if suffix.chars().all(|c| c.is_ascii_digit()) => base.to_string(),
        _ => name.to_string(),
    }
}

fn normalize_display_name(name: &str) -> String {
    let without_instance = trim_instance_suffix(name);
    without_instance.trim_end_matches("_C").to_string()
}

fn fallback_actor_identity(actor: *mut c_void) -> Option<ResolvedActor> {
    let class_name = object_class_name(actor)?;
    let object = object_name(actor).unwrap_or_else(|| class_name.clone());
    let lowered_class = class_name.to_ascii_lowercase();
    let lowered_object = object.to_ascii_lowercase();
    let is_bot = lowered_class.contains("bot") || lowered_object.contains("bot");

    Some(ResolvedActor {
        name: normalize_display_name(&object),
        is_bot,
    })
}

fn player_identity_from_player_state(player_state: *mut APlayerState) -> Option<ResolvedActor> {
    let player_state_ref = unsafe { player_state.as_ref() }?;
    let name = player_state_ref.player_name_private.copy_to_string().ok()?;
    let trimmed = name.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(ResolvedActor {
            name: trimmed.to_string(),
            is_bot: player_state_ref.player_flags.contains(PlayerFlags::IS_A_BOT),
        })
    }
}

fn player_identity_from_character(character: *mut ATBLCharacter) -> Option<ResolvedActor> {
    let character_ref = unsafe { character.as_ref() }?;
    let player_state = character_ref
        .base_tbl_character_base
        .base_character
        .base_pawn
        .player_state as *mut APlayerState;
    player_identity_from_player_state(player_state)
        .or_else(|| {
            let controller_ptr = character_ref.base_tbl_character_base.base_character.base_pawn.controller as *mut AController;
            player_identity_from_controller(controller_ptr)
        })
        .or_else(|| {
            // TBL-specific fallback: character tracks the previous owning player state.
            let last_player_state = character_ref.last_player_state as *mut APlayerState;
            player_identity_from_player_state(last_player_state)
        })
}

fn player_identity_from_controller(controller: *mut AController) -> Option<ResolvedActor> {
    let controller_ref = unsafe { controller.as_ref() }?;
    player_identity_from_player_state(controller_ref.player_state)
}

fn player_identity_from_actor(actor: *mut c_void) -> Option<ResolvedActor> {
    if object_inherits_from(actor, "TBLCharacter") || object_inherits_from(actor, "Character") {
        return player_identity_from_character(actor as *mut ATBLCharacter)
            .or_else(|| fallback_actor_identity(actor));
    }
    if object_inherits_from(actor, "Controller") {
        return player_identity_from_controller(actor as *mut AController)
            .or_else(|| fallback_actor_identity(actor));
    }
    if object_inherits_from(actor, "PlayerState") {
        return player_identity_from_player_state(actor as *mut APlayerState)
            .or_else(|| fallback_actor_identity(actor));
    }
    fallback_actor_identity(actor)
}

fn actor_debug_label(actor: *mut c_void) -> String {
    if actor.is_null() {
        return "null".to_string();
    }
    let class_name = object_class_name(actor).unwrap_or_else(|| "UnknownClass".to_string());
    let object = object_name(actor).unwrap_or_else(|| "UnknownObject".to_string());
    format!("ptr={actor:p}, class={class_name}, object={object}")
}

fn resolve_killer_from_damage_event(damage_event: &FDamageTakenEvent) -> Option<ResolvedActor> {
    let candidates = [damage_event.damage_instigator, damage_event.damage_causer];
    candidates
        .into_iter()
        .filter(|ptr| !ptr.is_null())
        .find_map(|ptr| player_identity_from_actor(ptr).or_else(|| fallback_actor_identity(ptr)))
}

fn resolve_killer_from_death_event(death_event: &FDeathDamageTakenEvent) -> Option<ResolvedActor> {
    let from_killers = death_event
        .killers
        .as_slice()
        .iter()
        .copied()
        .filter(|ptr| !ptr.is_null())
        .find_map(|ptr| player_identity_from_actor(ptr).or_else(|| fallback_actor_identity(ptr)));

    from_killers.or_else(|| resolve_killer_from_damage_event(&death_event.damage_taken))
}

fn resolve_victim(victim_character: *mut ATBLCharacter, damage_event: &FDamageTakenEvent) -> Option<ResolvedActor> {
    player_identity_from_character(victim_character).or_else(|| {
        let taker = damage_event.damage_taker;
        if taker.is_null() {
            None
        } else {
            player_identity_from_actor(taker)
        }
    })
}

fn to_combat_actor(actor: &ResolvedActor) -> CombatActor {
    CombatActor::new(actor.name.clone(), actor.is_bot)
}

fn damage_source_name(damage_event: &FDamageTakenEvent) -> String {
    let source_ptr = damage_event.damage_source as *mut c_void;
    if source_ptr.is_null() {
        return "UnknownDamageSource".to_string();
    }

    object_name(source_ptr)
        .or_else(|| object_class_name(source_ptr))
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "UnknownDamageSource".to_string())
}

fn attack_type_from_damage_event(damage_event: &FDamageTakenEvent) -> String {
    if damage_event.b_suicide {
        return "Suicide".to_string();
    }
    if damage_event.b_back_stab {
        return "Backstab".to_string();
    }
    if damage_event.b_entered_kill_volume {
        return "KillVolume".to_string();
    }
    "Damage".to_string()
}


CREATE_HOOK!(ATBLCharacter__OnKilled, ACTIVE, NONE, (), (
    this_ptr: *mut ATBLCharacter,
    damage_event: *const FDeathDamageTakenEvent
), {
    if this_ptr.is_null() {
        crate::swarn!(f; "ATBLCharacter::OnKilled called with null this_ptr");
        CALL_ORIGINAL!(ATBLCharacter__OnKilled(this_ptr, damage_event));
        return;
    }

    if damage_event.is_null() {
        crate::swarn!(f; "ATBLCharacter::OnKilled called with null damage_event");
        CALL_ORIGINAL!(ATBLCharacter__OnKilled(this_ptr, damage_event));
        return;
    }

    let death_event = unsafe { &*damage_event };
    let damage_taken = &death_event.damage_taken;

    let victim_actor = resolve_victim(this_ptr, damage_taken).unwrap_or_else(|| ResolvedActor {
        name: "UnknownVictim".to_string(),
        is_bot: false,
    });

    let killer_actor = resolve_killer_from_death_event(death_event).unwrap_or_else(|| {
        crate::sdebug!(
            f;
            "OnKilled unresolved killer: killers_len={} instigator=[{}], causer=[{}], taker=[{}]",
            death_event.killers.len(),
            actor_debug_label(damage_taken.damage_instigator),
            actor_debug_label(damage_taken.damage_causer),
            actor_debug_label(damage_taken.damage_taker)
        );
        ResolvedActor {
            name: "UnknownKiller".to_string(),
            is_bot: false,
        }
    });

    let source = damage_source_name(damage_taken);
    let attack_type = attack_type_from_damage_event(damage_taken);
    let kill_reason = death_event.kill_reason.as_str().to_string();

    crate::sdebug!(
        f;
        "OnKilled event: killer='{}' (bot={}) victim='{}' (bot={}) source='{}' reason='{}' damage={:.2}",
        killer_actor.name,
        killer_actor.is_bot,
        victim_actor.name,
        victim_actor.is_bot,
        source,
        kill_reason,
        damage_taken.damage
    );

    let killer_name = killer_actor.name.clone();
    let victim_name = victim_actor.name.clone();
    let killer_actor_event = to_combat_actor(&killer_actor);
    let victim_actor_event = to_combat_actor(&victim_actor);
    let mut killers = death_event
        .killers
        .as_slice()
        .iter()
        .copied()
        .filter_map(player_identity_from_actor)
        .map(|actor| to_combat_actor(&actor))
        .collect::<Vec<_>>();
    if killers.is_empty() {
        killers.push(killer_actor_event.clone());
    }

    EVENT_SYSTEM
        .game_event_publisher
        .publish(GameEvent::KillEvent(Kill {
            killer: killer_name.clone(),
            victim: victim_name.clone(),
            killer_actor: killer_actor_event.clone(),
            victim_actor: victim_actor_event.clone(),
            killers,
            kill_reason,
            random_seed: death_event.random_seed,
            dead_character_id: death_event.dead_character_id,
            attach_to_projectile: death_event.b_attach_to_projectile,
            source: Damage {
                attacker: killer_name,
                victim: victim_name,
                attacker_actor: killer_actor_event,
                victim_actor: victim_actor_event,
                damage: DamageSource {
                    amount: damage_taken.damage,
                    source,
                    attack_type,
                },
            },
        }));

    CALL_ORIGINAL!(ATBLCharacter__OnKilled(this_ptr, damage_event));
});
