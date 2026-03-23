use std::os::raw::c_void;

use crate::events::models::{CombatActor, Damage, DamageSource, GameEvent, Kill};
use crate::features::events::EVENT_SYSTEM;
use crate::game::chivalry2::{AController, APlayerState, ATBLCharacter, FDeathDamageTakenEvent, FDamageTakenEvent, PlayerFlags};
use crate::ue::{UObject, UStruct};

define_pattern_resolver!(ATBLCharacter__OnKilled, [
    "4C 8B DC 55 41 55 41 57 49 8D AB ?? ?? ?? ?? 48 81 EC 30 03 00 00"
]);

fn object_class_name(ptr: *mut c_void) -> Option<String> {
    let obj = unsafe { (ptr as *const UObject).as_ref() }?;
    let class = unsafe { obj.uobject_base_utility.uobject_base.class_private.as_ref() }?;
    Some(class.ustruct.ufield.uobject.uobject_base_utility.uobject_base.name_private.to_string())
}

fn object_name(ptr: *mut c_void) -> Option<String> {
    let obj = unsafe { (ptr as *const UObject).as_ref() }?;
    Some(obj.uobject_base_utility.uobject_base.name_private.to_string())
}

fn object_inherits_from(ptr: *mut c_void, needle: &str) -> bool {
    let mut curr = unsafe { (ptr as *const UObject).as_ref() }
        .and_then(|o| unsafe { o.uobject_base_utility.uobject_base.class_private.as_ref() })
        .map(|c| &c.ustruct as *const UStruct);

    while let Some(s) = unsafe { curr.and_then(|p| p.as_ref()) } {
        if s.ufield.uobject.uobject_base_utility.uobject_base.name_private.to_string().contains(needle) {
            return true;
        }
        curr = (!s.super_struct.is_null()).then_some(s.super_struct);
    }
    false
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

fn fallback_combat_actor(ptr: *mut c_void) -> Option<CombatActor> {
    let cls = object_class_name(ptr)?;
    let obj = object_name(ptr).unwrap_or_else(|| cls.clone());
    let is_bot = cls.to_lowercase().contains("bot") || obj.to_lowercase().contains("bot");

    Some(CombatActor::new(normalize_display_name(&obj), is_bot))
}

fn combat_actor_from_player_state(player_state: *mut APlayerState) -> Option<CombatActor> {
    let ps = unsafe { player_state.as_ref() }?;
    let name = ps.player_name_private.copy_to_string().ok()?;
    (!name.trim().is_empty()).then(|| {
        CombatActor::new(name.trim().to_string(), ps.player_flags.contains(PlayerFlags::IS_A_BOT))
    })
}

fn combat_actor_from_actor(ptr: *mut c_void) -> Option<CombatActor> {
    if ptr.is_null() { return None; }

    if object_inherits_from(ptr, "TBLCharacter") || object_inherits_from(ptr, "Character") {
        let char = unsafe { (ptr as *mut ATBLCharacter).as_ref() }?;
        let pawn = &char.base_tbl_character_base.base_character.base_pawn;

        return combat_actor_from_player_state(pawn.player_state as *mut APlayerState)
            .or_else(|| {
                let controller = unsafe { pawn.controller.cast::<AController>().as_ref() }?;
                combat_actor_from_player_state(controller.player_state)
            })
            .or_else(|| combat_actor_from_player_state(char.last_player_state as *mut APlayerState))
            .or_else(|| fallback_combat_actor(ptr));
    }

    if object_inherits_from(ptr, "Controller") {
        let controller = unsafe { (ptr as *mut AController).as_ref() }?;
        return combat_actor_from_player_state(controller.player_state)
            .or_else(|| fallback_combat_actor(ptr));
    }

    if object_inherits_from(ptr, "PlayerState") {
        return combat_actor_from_player_state(ptr.cast())
            .or_else(|| fallback_combat_actor(ptr));
    }

    fallback_combat_actor(ptr)
}

fn resolve_killer_from_damage_event(damage_event: &FDamageTakenEvent) -> Option<CombatActor> {
    combat_actor_from_actor(damage_event.damage_instigator.cast())
        .or_else(|| combat_actor_from_actor(damage_event.damage_causer.cast()))
}

fn resolve_killers_from_death_event(death_event: &FDeathDamageTakenEvent) -> Vec<CombatActor> {
    let mut resolved: Vec<_> = death_event.killers.as_slice().iter()
        .filter_map(|&ptr| combat_actor_from_actor(ptr))
        .collect();

    if resolved.is_empty() {
        if let Some(actor) = resolve_killer_from_damage_event(&death_event.damage_taken) {
            resolved.push(actor);
        }
    }
    resolved
}

fn resolve_victim(damage_event: &FDamageTakenEvent) -> Option<CombatActor> {
    combat_actor_from_actor(damage_event.damage_taker.cast())
}

fn damage_source_name(damage_event: &FDamageTakenEvent) -> String {
    let ptr = damage_event.damage_source as *mut c_void;
    if ptr.is_null() {
        return "UnknownDamageSource".to_string();
    }

    object_name(ptr)
        .or_else(|| object_class_name(ptr))
        .map(|name| normalize_display_name(&name))
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "UnknownDamageSource".to_string())
}

fn attack_type_from_damage_event(damage_event: &FDamageTakenEvent) -> String {
    if damage_event.b_suicide { return "Suicide".into(); }
    if damage_event.b_back_stab { return "Backstab".into(); }
    if damage_event.b_entered_kill_volume { return "KillVolume".into(); }
    "Damage".into()
}


CREATE_HOOK!(ATBLCharacter__OnKilled, ACTIVE, NONE, (), (
    this_ptr: *mut ATBLCharacter,
    damage_event: *const FDeathDamageTakenEvent
), {
    let death_event = unsafe { damage_event.as_ref() };
    if this_ptr.is_null() || death_event.is_none() {
        CALL_ORIGINAL!(ATBLCharacter__OnKilled(this_ptr, damage_event));
        return;
    }
    let death_event = death_event.unwrap();
    let damage_taken = &death_event.damage_taken;

    let victim = resolve_victim(damage_taken)
        .unwrap_or_else(|| CombatActor::new("UnknownVictim".into(), false));

    let killers = resolve_killers_from_death_event(death_event);
    let killer = killers.first().cloned().unwrap_or_else(|| CombatActor::new("UnknownKiller".into(), false));

    let source = damage_source_name(damage_taken);
    let kill_reason = death_event.kill_reason.as_str().to_string();

    crate::sdebug!(f; "Kill: {} -> {} [{}] via {} ({:.2} dmg)", killer.name, victim.name, kill_reason, source, damage_taken.damage);

    EVENT_SYSTEM.game_event_publisher.publish(GameEvent::KillEvent(Kill {
        killer: killer.clone(),
        victim: victim.clone(),
        killers: if killers.is_empty() { vec![killer.clone()] } else { killers },
        kill_reason,
        random_seed: death_event.random_seed,
        dead_character_id: death_event.dead_character_id,
        attach_to_projectile: death_event.b_attach_to_projectile,
        source: Damage {
            attacker: killer.name.clone(),
            victim: victim.name.clone(),
            attacker_actor: killer,
            victim_actor: victim,
            damage: DamageSource {
                amount: damage_taken.damage,
                source,
                attack_type: attack_type_from_damage_event(damage_taken),
            },
        },
    }));

    CALL_ORIGINAL!(ATBLCharacter__OnKilled(this_ptr, damage_event));
});
