use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Instant;
use crate::events::broadcast::BroadcastMessage;
use crate::events::bus::{EventPublisher, Subscriber};
use crate::events::models::GameEvent;

struct ActiveDuel {
    p1: String,
    p2: String,
    start_time: Instant,
    damage_dealt: HashMap<String, f32>,
    attack_counts: HashMap<String, HashMap<String, u32>>,
    parries: HashMap<String, u32>,
}

enum DuelState {
    Idle,
    Active(ActiveDuel),
}

pub struct DuelManagerSubscriber {
    state: DuelState,
    broadcaster: &'static EventPublisher<BroadcastMessage>,
}

impl DuelManagerSubscriber {
    pub fn new(broadcaster: &'static EventPublisher<BroadcastMessage>) -> Self {
        Self {
            state: DuelState::Idle,
            broadcaster,
        }
    }

    fn format_results(&self, duel: &ActiveDuel, winner: &str, loser: &str) -> BroadcastMessage {
        let duration = duel.start_time.elapsed().as_secs();
        let mut msg = BroadcastMessage::new()
            .title("?? DUEL CONCLUDED".to_string())
            .color(0xe67e22)
            .content(format!("{} has defeated {}!", winner, loser))
            .field("Duration".to_string(), format!("{} seconds", duration));

        for player in [&duel.p1, &duel.p2] {
            let dmg = duel.damage_dealt.get(player).unwrap_or(&0.0);
            let parries = duel.parries.get(player).unwrap_or(&0);
            let attacks = duel.attack_counts.get(player);
            
            let mut tech_stats = format!("Total Dmg: {:.0}\nParries: {}\n", dmg, parries);
            if let Some(counts) = attacks {
                for (style, count) in counts {
                    tech_stats.push_str(&format!("{}: {}\n", style, count));
                }
            }
            
            msg = msg.field(format!("Stats: {}", player), tech_stats);
        }

        msg
    }

    fn handle_active_event(&mut self, duel: &mut ActiveDuel, event: &GameEvent) {
        match event {
            GameEvent::AttackEvent(e) => {
                if e.attacker == duel.p1 || e.attacker == duel.p2 {
                    let p_counts = duel.attack_counts.entry(e.attacker.clone()).or_default();
                    *p_counts.entry(e.attack_type.clone()).or_insert(0) += 1;

                    if e.was_parried {
                        let defender = if e.attacker == duel.p1 { &duel.p2 } else { &duel.p1 };
                        *duel.parries.entry(defender.clone()).or_insert(0) += 1;
                    }
                }
            }
            GameEvent::DamageEvent(e) => {
                let is_p1 = e.victim == duel.p1;
                let is_p2 = e.victim == duel.p2;

                if is_p1 || is_p2 {
                    *duel.damage_dealt.entry(e.attacker.clone()).or_insert(0.0) += e.damage.amount;
                } else if e.attacker == duel.p1 || e.attacker == duel.p2 {
                    self.state = DuelState::Idle;
                    self.broadcaster.publish(BroadcastMessage::from("?? **Duel Cancelled**: Interference detected!"));
                }
            }
            GameEvent::KillEvent(e) => {
                if (e.victim.name == duel.p1 && e.killer.name == duel.p2)
                    || (e.victim.name == duel.p2 && e.killer.name == duel.p1)
                {
                    let results = self.format_results(duel, &e.killer.name, &e.victim.name);
                    self.state = DuelState::Idle;
                    self.broadcaster.publish(results);
                }
            }
            _ => {}
        }
    }
}

#[async_trait]
impl Subscriber<GameEvent> for DuelManagerSubscriber {
    fn identifier(&self) -> &'static str {
        "DuelManagerSubscriber"
    }

    async fn on_event(&mut self, event: &GameEvent) {
        match event {
            GameEvent::DuelStartEvent(e) => {
                self.state = DuelState::Active(ActiveDuel {
                    p1: e.challenger.clone(),
                    p2: e.opponent.clone(),
                    start_time: Instant::now(),
                    damage_dealt: HashMap::new(),
                    attack_counts: HashMap::new(),
                    parries: HashMap::new(),
                });
                self.broadcaster.publish(BroadcastMessage::from(format!("?? **DUEL STARTED**: {} vs {}!", e.challenger, e.opponent)));
            }
            _ => {
                if let DuelState::Active(mut duel) = std::mem::replace(&mut self.state, DuelState::Idle) {
                    self.handle_active_event(&mut duel, event);
                    if let DuelState::Idle = self.state {
                        // Already handled transition to Idle in handle_active_event
                    } else {
                        self.state = DuelState::Active(duel);
                    }
                }
            }
        }
    }

    async fn on_tick(&mut self) {}
}
