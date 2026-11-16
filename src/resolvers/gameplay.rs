use crate::events::models::{CombatActor, Damage, DamageSource, GameEvent, Kill};
use crate::features::events::EVENT_SYSTEM;
use crate::game::chivalry2::{ACharacter, AController, APlayerState, ATBLCharacter, FDeathDamageTakenEvent, FDamageTakenEvent, PlayerFlags};
use crate::ue::{UObject, UObjectType};

define_pattern_resolver!(APlayerState_GetPlayerName, Call, [
    "48 8D 55 E7 48 8B CB E8 | ?? ?? ?? ?? 83 78 08 00 74"
]);
CREATE_HOOK!(APlayerState_GetPlayerName, CALLED, *mut crate::ue::FString,
    (player: *const APlayerState, output: *mut crate::ue::FString), {});

fn fallback_combat_actor(ptr: *mut UObject) -> Option<CombatActor> {
    let object = unsafe { UObject::from_ptr(ptr) }?;
    let cls = unsafe { object.class_name() }?;
    let name = object.display_name();
    let is_bot = cls.to_lowercase().contains("bot") || name.to_lowercase().contains("bot");

    Some(CombatActor::new(name, is_bot))
}

/// `via` names the field the pointer came from, for the warning when it is not a live object.
fn combat_actor_from_player_state(player_state: *mut APlayerState, via: &str) -> Option<CombatActor> {
    if player_state.is_null() { return None; }
    let Some(ps) = (unsafe { APlayerState::from_ptr(player_state) }) else {
        swarn!("ignoring PlayerState {:p} from {}: not a live PlayerState", player_state, via);
        return None;
    };
    let name = ps.player_name_private.copy_to_string().ok()?;
    (!name.trim().is_empty()).then(|| {
        CombatActor::new(name.trim().to_string(), ps.player_flags.contains(PlayerFlags::IS_A_BOT))
    })
}

fn combat_actor_from_actor(ptr: *mut UObject) -> Option<CombatActor> {
    if ptr.is_null() { return None; }
    let Some(object) = (unsafe { UObject::from_ptr(ptr) }) else {
        swarn!("ignoring combat actor {:p}: not a live object", ptr);
        return None;
    };

    if let Some(character) = unsafe { object.uobj_cast::<ACharacter>() } {
        let pawn = &character.base_pawn;

        return combat_actor_from_player_state(pawn.player_state as *mut APlayerState, "Pawn.PlayerState")
            .or_else(|| {
                let controller = unsafe { AController::from_ptr(pawn.controller) }?;
                combat_actor_from_player_state(controller.player_state, "Pawn.Controller.PlayerState")
            })
            .or_else(|| {
                let character = unsafe { object.uobj_cast::<ATBLCharacter>() }?;
                combat_actor_from_player_state(character.last_player_state as *mut APlayerState, "TBLCharacter.LastPlayerState")
            })
            .or_else(|| fallback_combat_actor(ptr));
    }

    if let Some(controller) = unsafe { object.uobj_cast::<AController>() } {
        return combat_actor_from_player_state(controller.player_state, "Controller.PlayerState")
            .or_else(|| fallback_combat_actor(ptr));
    }

    if unsafe { object.uobj_cast::<APlayerState>() }.is_some() {
        return combat_actor_from_player_state(ptr.cast(), "PlayerState")
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
        .filter_map(|&ptr| combat_actor_from_actor(ptr.cast()))
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
    // Prefer concrete weapon/projectile identity over abstract UDamageSource.
    let candidates = [
        (
            "inventory_item",
            damage_event.inventory_item.cast::<UObject>(),
        ),
        ("projectile", damage_event.projectile.cast::<UObject>()),
        (
            "damage_source",
            damage_event.damage_source.cast::<UObject>(),
        ),
    ];

    for (kind, ptr) in candidates {
        if ptr.is_null() {
            continue;
        }
        if let Some(object) = unsafe { UObject::from_ptr(ptr) } {
            let name = object.display_name();
            if name.trim().is_empty() { continue; }
            if kind != "damage_source" {
                strace!("Kill source resolved via {}='{}'", kind, name);
            }
            return name;
        }
    }

    "UnknownDamageSource".to_string()
}

fn attack_type_from_damage_event(damage_event: &FDamageTakenEvent) -> String {
    if damage_event.b_suicide { return "Suicide".into(); }
    if damage_event.b_back_stab { return "Backstab".into(); }
    if damage_event.b_entered_kill_volume { return "KillVolume".into(); }
    "Damage".into()
}

define_pattern_resolver!(ATBLCharacter__OnKilled, [
    "4C 8B DC 55 41 55 41 57 49 8D AB ?? ?? ?? ?? 48 81 EC 30 03 00 00"
]);
CREATE_HOOK!(ATBLCharacter__OnKilled, ACTIVE, NONE, (), (
    this_ptr: *mut ATBLCharacter,
    damage_event: *const FDeathDamageTakenEvent
), {
    if this_ptr.is_null() || damage_event.is_null() {
        CALL_ORIGINAL!(ATBLCharacter__OnKilled(this_ptr, damage_event));
        return;
    }
    let death_event = unsafe { &*damage_event };
    let damage_taken = &death_event.damage_taken;

    let victim = resolve_victim(damage_taken)
        .unwrap_or_else(|| CombatActor::new("UnknownVictim".into(), false));

    let killers = resolve_killers_from_death_event(death_event);
    let killer = killers.first().cloned().unwrap_or_else(|| CombatActor::new("UnknownKiller".into(), false));

    let source = damage_source_name(damage_taken);
    let kill_reason = death_event.kill_reason.as_str().to_string();

    strace!("Kill: {} -> {} [{}] via {} ({:.2} dmg)", killer.name, victim.name, kill_reason, source, damage_taken.damage);

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


define_pattern_resolver!(ATBLCharacter__OnDamageTaken, [
    "4C 8B DC 55 57 41 55 41 56 49 8D AB ?? ?? ?? ?? 48 81 EC 58 02 00 00"
]);
CREATE_HOOK!(ATBLCharacter__OnDamageTaken, ACTIVE, NONE, (), (
    this_ptr: *mut ATBLCharacter,
    damage_event: *const FDamageTakenEvent
), {
    if this_ptr.is_null() || damage_event.is_null() {
        CALL_ORIGINAL!(ATBLCharacter__OnDamageTaken(this_ptr, damage_event));
        return;
    }

    let damage_taken = unsafe { &*damage_event };
    let attacker = resolve_killer_from_damage_event(damage_taken)
        .unwrap_or_else(|| CombatActor::new("UnknownAttacker".into(), false));
    let victim = resolve_victim(damage_taken)
        .or_else(|| combat_actor_from_actor(this_ptr.cast()))
        .unwrap_or_else(|| CombatActor::new("UnknownVictim".into(), false));

    let source = damage_source_name(damage_taken);
    let attack_type = attack_type_from_damage_event(damage_taken);

    strace!(
        "Damage: {} -> {} via {} ({:.2} dmg, type={})",
        attacker.name,
        victim.name,
        source,
        damage_taken.damage,
        attack_type
    );

    EVENT_SYSTEM.game_event_publisher.publish(GameEvent::DamageEvent(Damage {
        attacker: attacker.name.clone(),
        victim: victim.name.clone(),
        attacker_actor: attacker,
        victim_actor: victim,
        damage: DamageSource {
            amount: damage_taken.damage,
            source,
            attack_type,
        },
    }));

    CALL_ORIGINAL!(ATBLCharacter__OnDamageTaken(this_ptr, damage_event));
});
