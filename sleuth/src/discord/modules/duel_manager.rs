use serenity::all::ChannelId;
use serenity::all::CreateEmbed;
use serenity::all::CreateMessage;
use serenity::all::Http;

use crate::discord::core::*;
use crate::discord::notifications::*;
use crate::discord::responses::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Instant, Duration};

struct ActiveDuel {
    p1: String,
    p2: String,
    start_time: Instant,
    damage_dealt: HashMap<String, f32>,
    attack_counts: HashMap<String, HashMap<String, u32>>, // Name -> (Type -> Count)
    parries: HashMap<String, u32>,
}

pub struct DuelManager {
    current_duel: Option<ActiveDuel>,
}

impl DuelManager {
    pub fn new() -> Self { Self { current_duel: None } }
}

#[async_trait::async_trait]
impl DiscordSubscriber for DuelManager {
    fn name(&self) -> &'static str { "DuelManager" }

    async fn on_event(&mut self, event: &dyn GameEvent, _http: &Arc<Http>, _channel: ChannelId) -> Vec<BotResponse> {
        let any = event.as_any();

        // 1. START DUEL
        if let Some(e) = any.downcast_ref::<DuelStartEvent>() {
            self.current_duel = Some(ActiveDuel {
                p1: e.challenger.clone(),
                p2: e.opponent.clone(),
                start_time: Instant::now(),
                damage_dealt: HashMap::new(),
                attack_counts: HashMap::new(),
                parries: HashMap::new(),
            });
            return msg(format!("⚔️ **DUEL STARTED**: {} vs {}!", e.challenger, e.opponent)).to_main().into_responses();
        }

        // 2. TRACK ATTACKS & PARRIES
        if let Some(duel) = &mut self.current_duel {
            if let Some(e) = any.downcast_ref::<AttackEvent>() {
                if e.attacker == duel.p1 || e.attacker == duel.p2 {
                    let p_counts = duel.attack_counts.entry(e.attacker.clone()).or_default();
                    *p_counts.entry(e.attack_type.clone()).or_insert(0) += 1;
                    
                    if e.was_parried {
                        let defender = if e.attacker == duel.p1 { &duel.p2 } else { &duel.p1 };
                        *duel.parries.entry(defender.clone()).or_insert(0) += 1;
                    }
                }
            }

            // 3. TRACK DAMAGE & END DUEL (When someone dies/wins)
            if let Some(e) = any.downcast_ref::<DamageEvent>() {
                let is_p1 = e.victim == duel.p1;
                let is_p2 = e.victim == duel.p2;

                if is_p1 || is_p2 {
                    *duel.damage_dealt.entry(e.attacker.clone()).or_insert(0.0) += e.damage;
                    
                    // Logic: If damage kills them (or is the 'final blow' event)
                    // For this example, let's assume a "DeathEvent" is handled elsewhere 
                    // or we check if damage > 100.
                } else if e.attacker == duel.p1 || e.attacker == duel.p2 {
                    // "Interference" rule: If a duelist hits a random player, cancel the duel?
                    self.current_duel = None;
                    return msg("🚫 **Duel Cancelled**: Interference detected!").to_main().into_responses();
                }
            }
            
            // 4. WIN CONDITION (Example: listening for KillEvent)
            if let Some(e) = any.downcast_ref::<KillEvent>() {
                // Check if we even have a duel active first
                if let Some(duel) = &self.current_duel {
                    if (e.victim == duel.p1 && e.killer == duel.p2) || (e.victim == duel.p2 && e.killer == duel.p1) {
                        
                        // FIX: Remove the duel from self.current_duel. 
                        // This yields ownership of the 'duel' data and 'un-borrows' self.
                        if let Some(finished_duel) = self.current_duel.take() {
                            let msg = self.format_results(&finished_duel, &e.killer, &e.victim);
                            return BotResponse::from(msg).into_responses();
                        }
                    }
                }
            }
        }
        NO_RESP
    }
}

impl DuelManager {
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