use serenity::all::ChannelId;
use serenity::all::CreateEmbed;
use serenity::all::CreateMessage;
use serenity::all::Http;

use crate::discord::core::*;
use crate::discord::events::*;
use crate::discord::responses::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

struct ActiveDuel {
    p1: String,
    p2: String,
    start_time: Instant,
    damage_dealt: HashMap<String, f32>,
    attack_counts: HashMap<String, HashMap<String, u32>>, // Name -> (Type -> Count)
    parries: HashMap<String, u32>,
}

enum DuelState {
    Idle,
    Active(ActiveDuel),
}

pub struct DuelManager {
    state: DuelState,
}

impl DuelManager {
    pub fn new() -> Self { Self { state: DuelState::Idle } }
}

#[async_trait::async_trait]
impl DiscordSubscriber for DuelManager {
    fn name(&self) -> &'static str { "DuelManager" }

    async fn on_event(&mut self, event: &GameEvent, _http: &Arc<Http>, _channel: ChannelId) -> Vec<BotResponse> {
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
                msg(format!("⚔️ **DUEL STARTED**: {} vs {}!", e.challenger, e.opponent))
                    .to_main()
                    .into_responses()
            }
            _ => {
                if let DuelState::Active(mut duel) =
                    std::mem::replace(&mut self.state, DuelState::Idle)
                {
                    let responses = self.handle_active_event(&mut duel, event);
                    // If handler didn't transition state to Idle itself, we put it back.
                    if let DuelState::Idle = self.state {
                        if responses.is_empty() {
                            self.state = DuelState::Active(duel);
                        }
                    }
                    responses
                } else {
                    NO_RESP
                }
            }
        }
    }
}

impl DuelManager {
    fn handle_active_event(&mut self, duel: &mut ActiveDuel, event: &GameEvent) -> Vec<BotResponse> {
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
                    *duel.damage_dealt.entry(e.attacker.clone()).or_insert(0.0) += e.damage;
                } else if e.attacker == duel.p1 || e.attacker == duel.p2 {
                    self.state = DuelState::Idle;
                    return msg("🚫 **Duel Cancelled**: Interference detected!").to_main().into_responses();
                }
            }
            GameEvent::KillEvent(e) => {
                if (e.victim == duel.p1 && e.killer == duel.p2) || (e.victim == duel.p2 && e.killer == duel.p1) {
                    self.state = DuelState::Idle;
                    return BotResponse::from(self.format_results(duel, &e.killer, &e.victim)).into_responses();
                }
            }
            _ => {}
        }

        NO_RESP
    }
    fn format_results(&self, duel: &ActiveDuel, winner: &str, loser: &str) -> CreateMessage {
        let duration = duel.start_time.elapsed().as_secs();
        let mut embed = CreateEmbed::new()
            .title("💀 DUEL CONCLUDED")
            .color(0xe67e22)
            .description(format!("**{}** has defeated **{}**!", winner, loser))
            .field("Duration", format!("{} seconds", duration), true);

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
            
            embed = embed.field(format!("Stats: {}", player), tech_stats, true);
        }

        CreateMessage::new().add_embed(embed)
    }
}