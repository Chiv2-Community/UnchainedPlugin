use crate::discord::config::DiscordConfig;
use crate::discord::config::ModuleConfig;
use crate::discord::core::DiscordSubscriber;
use crate::discord::notifications::GameEvent;
use crate::discord::responses::*;
use crate::discord::notifications::*;
use crate::game::chivalry2::ATBLGameMode;
use crate::game::chivalry2::PlayerFlags;
use crate::game::engine::UWorld;
use crate::sinfo;
use itertools::Itertools;
use rand::seq::IndexedRandom;
use serde::Deserialize;
use serde::Serialize;
use serenity::all::{Http, ChannelId, CreateMessage, MessageId, CreateEmbed, EditMessage};
use sleuth_macros::handler_command;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Instant, Duration};
use crate::resolvers::etc_hooks::o_GetTBLGameMode;

pub struct Dashboard {
    // Current State
    player_count: u32,
    current_map: String,
    last_update: Instant,
    
    // Discord Reference
    message_id: Option<MessageId>,
    message_id2: Option<MessageId>,
    needs_refresh: bool,
    status: Option<ServerStatus>,

    _ctx: crate::discord::Ctx,
    settings: DashboardSettings,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct DashboardSettings {
    auto_spawn_dash: bool,
}

impl Default for DashboardSettings {
    fn default() -> Self {
        Self { 
            auto_spawn_dash: false
        }
    }
}

impl ModuleConfig for DashboardSettings {
    fn key() -> &'static str { "Dashboard" }
}


impl Dashboard {
    pub fn new(ctx: crate::discord::Ctx) -> Self {
        let settings = ctx.config.get_module_config::<DashboardSettings>().unwrap_or_default();
        Self {
            player_count: 0,
            current_map: "Loading...".to_string(),
            last_update: Instant::now(),
            message_id: None,
            message_id2: None,
            needs_refresh: settings.auto_spawn_dash,
            status: None,
            _ctx: ctx,
            settings,
        }
    }

    // fn build_embed(&self) -> CreateEmbed {
    //     CreateEmbed::new()
    //         .title("🏰 Server Live Dashboard")
    //         .color(0x3498db)
    //         .field("Status", "🟢 Online", true)
    //         .field("Map", &self.current_map, true)
    //         .field("Players", format!("{}/64", self.player_count), true)
    //         .footer(serenity::all::CreateEmbedFooter::new(
    //             format!("Last updated: {:?}", self.last_update.elapsed())
    //         ))
    // }


    fn build_embed(&self) -> CreateEmbed {
        let cur_status = self.status.clone().expect("No status available");
        
        let active_names: HashSet<_> = cur_status.active_mods
            .iter()
            .map(|m| &m.name)
            .collect();

        let mod_list = match cur_status.active_mods.as_slice() {
            [] => "None".to_string(),
            active_mods => active_mods
                .iter()
                .map(|m| format!("{} *({})*", m.name, m.version))
                .collect::<Vec<_>>()
                .join("\n- "),
        };

        let all_mod_list = match cur_status.mods.as_slice() {
            [] => "None".to_string(),
            mods => {
                let list = mods
                    .iter()
                    .filter(|m| !active_names.contains(&m.name))
                    .map(|m| format!("{} *({})*", m.name, m.version))
                    .collect::<Vec<_>>()
                    .join(", ");

                if list.is_empty() { "None".to_string() } else { format!("-# {list}") }
            }
        };

        // The top text (Description) contains the build info and server type
        let description = format!(
            "{}",
            cur_status.description, // "Chivalry 2 (build 261891)"
        );

        CreateEmbed::new()
            .title(format!("🌐 {}", cur_status.name))
            .color(0x2B2D31)
            .description(description)
            .field("Map", &cur_status.current_map, true)
            .field("Players", format!("{}/{}", cur_status.player_count, cur_status.max_players), true)
            .field("Active Mods", mod_list, false)
            .field("All Mods", all_mod_list, false)
    }
    
    #[handler_command("cta", desc="Issue a Call to Arms on the discord server", source = "GameChat")]
    pub fn cmd_cta(&mut self, message: String, cmd: &GameCommandEvent) -> Vec<BotResponse> {
        if self.status.as_ref().is_none() {
            return NO_RESP;
        }
        let cur_status = self.status.clone().expect("No status available");
        let mod_list = match cur_status.active_mods.as_slice() {
            [] => "None".to_string(),
            active_mods => format!(
                "- {}",
                active_mods
                    .iter()
                    .map(|m| format!("**{}** *({})*", m.name, m.version))
                    .collect::<Vec<_>>()
                    .join("\n- ")
            ),
        };
        
        let sender = &cmd.actor.display_name;

        let templates = [
            // The Classic
            format!("**{}** has issued a __**Call to Arms**__!\nJoinEvent the server and fight for your honor!\nMessage: _{}_", sender, message),
            format!("⚠️ **REINFORCEMENTS NEEDED!**\n**{}** is requesting immediate backup.\nOrders: _{}_", sender, message),
            format!("📢 **BANNERS RAISED!**\n**{}** has sounded the war horn! Rally to their side!\nWar Cry: _{}_", sender, message),                    
            format!("🔥 **TO THE FRONT LINES!**\n**{}** says: _{}_\nDon't let them stand alone!", sender, message),
            format!("⚔️ **{}** is calling for all able-bodied warriors!\n> _{}_", sender, message),
            format!("🍖 **FRESH MEAT!**\n**{}** is getting beat up and needs someone to hide behind. JoinEvent now!\nExcuse: *\"{}\"*", sender, message),
            format!("🕹️ **STOP SLACKING!**\n**{}** has issued a Call to Arms. Your couch can wait, the server can't!\nMessage: _{}_", sender, message),
            format!("📉 **STONKS ARE DOWN!**\n**{}** says the kill count is too low. Let's pump those numbers up!\nMemo: *\"{}\"*", sender, message),
            format!("⚠️ **BROKEN ARROW!**\n**{}** is being overrun and has declared a Level 5 Emergency!\nComms: _{}_", sender, message),
            format!("🚁 **REINFORCEMENTS REQ: IMMEDIATE**\n**{}** is popping smoke. ETA on your arrival?\nIntel: *\"{}\"*", sender, message),
            format!("👊 **SQUAD UP!**\n**{}** is tired of fighting alone. Get in there and provide some fire support!\nNote: _{}_", sender, message),
            format!("⚔️ **THE BANNERS ARE RAISED!**\n**{}** has sounded the Great Horn of Battle! Will you answer the call?\nWar Cry: *\"{}\"*", sender, message),
            format!("🛡️ **TO GLORY!**\n**{}** is leading a charge and demands your presence on the field!\nOrders: _{}_", sender, message),
            format!("🏰 **DEFEND THE REALM!**\n**{}** reports that the front lines are thinning. Rally to the server!\nStatus: *\"{}\"*", sender, message),
        ];

        let colors = [
            0xe67e22,
            0xe74c3c,
            0xf1c40f,
        ];

        let mut rng = rand::rng();
        let chosen_color = *colors.choose(&mut rng).unwrap_or(&0xe67e22);
        let chosen_description = templates.choose(&mut rng).unwrap_or(&templates[0]).to_string();
        
        let embed = CreateEmbed::new()
            .title("⚔️ CALL TO ARMS ⚔️")
            .color(chosen_color)
            .description(chosen_description)
            .field("Server", cur_status.name, false)
            .field("Description", cur_status.description, false)
            .field("Current Map", cur_status.current_map, true)
            .field("All Mods", mod_list, true);
        BotResponse::from(embed).to_main().into_responses() // FIXME: also write to general
    }
    
    #[handler_command("dash", desc="Display server dashboard (if available)", source = "Discord")]
    pub fn cmd_dash(&mut self, _cmd: &GameCommandEvent) -> Vec<BotResponse> {        
        if self.status.is_none() {
            return msg("Dashboard: no server status available").into_responses();
        }
        else {
            self.message_id = None; // Resetting this forces a new message on next tick
            self.message_id2 = None; // Resetting this forces a new message on next tick
            self.needs_refresh = true;
        }
        NO_RESP
    }
    
    #[handler_command("playerlist", desc="Display server dashboard (if available)")]
    pub fn cmd_playerlist(&mut self, _cmd: &GameCommandEvent) -> Vec<BotResponse> {
        if let Some(world) = crate::globals().world() {
            let game_ptr: *mut ATBLGameMode = CALL_ORIGINAL!(GetTBLGameMode(world));
            let game = unsafe {game_ptr.as_mut().expect("GameMode was null")};

            
            sinfo!(f; "Name:{}", game.server_name);
            let maplist = game.maplist.as_slice().iter().join(", ");
            sinfo!(f; "MapList:{}", maplist);
            sinfo!(f; "Idle disconnect:{}", game.idle_kick_timer_disconnect);
            sinfo!(f; "Idle spectate:{}", game.idle_kick_timer_spectate);

            let uworld_ptr = world as *mut UWorld;
            let uworld = unsafe {uworld_ptr.as_mut().expect("World was null")};

            let game_state = unsafe {(uworld.game_state).as_mut().expect("GameState was null")};
            let mut table_content = String::from("```rust\n");
            table_content.push_str("NAME           | K/D/A    | SCORE | FLAGS\n");
            table_content.push_str("---------------|----------|-------|-------\n");

            for player_raw in game_state.player_array.as_mut_slice() {
                let player_state = unsafe { player_raw.as_mut().expect("PlayerState was null") };
                
                let mut flags = Vec::new();
                if player_state.base.player_flags.contains(PlayerFlags::IS_SPECTATOR) {
                    flags.push("SPEC");
                }
                if player_state.base.player_flags.contains(PlayerFlags::ONLY_SPECTATOR) {
                    flags.push("OSPEC");
                }
                if player_state.base.player_flags.contains(PlayerFlags::IS_A_BOT) {
                    flags.push("BOT");
                }
                let flag_str = flags.join("|");

                let full_ip = player_state.base.saved_network_address.to_string(); // Assuming FString to String
                let _short_ip = if full_ip.len() >= 4 { &full_ip[..4] } else { &full_ip };

                let raw_name = player_state.base.player_name_private.to_string();
                let display_name = if raw_name.len() > 14 { format!("{}..", &raw_name[..12]) } else { raw_name };

                table_content.push_str(&format!(
                    "{:<14} | {:>2}/{:>2}/{:>2} | {:>5} | {} ({:?})\n",
                    display_name,
                    player_state.kills,
                    player_state.deaths,
                    player_state.assists,
                    player_state.player_score,
                    flag_str,
                    player_state.base.player_flags
                ));
            }

            table_content.push_str("```");

            let embed = CreateEmbed::default()
                .title("Match Game State")
                .description(table_content)
                .color(0x2f3136) // Dark theme color
                .timestamp(serenity::model::Timestamp::now())
                .field("Server", format!("{}", game.server_name), false)
                .field("Map", format!("{}", maplist), false);

            return BotResponse::from(embed).to_main().into_responses()

        }
        NO_RESP
    }

}

#[async_trait::async_trait]
impl DiscordSubscriber for Dashboard {
    fn get_commands(&self) -> Vec<CommandInfo> {
        crate::auto_help!(self, [
            cmd_cta,
            cmd_playerlist,
            cmd_dash
        ])
    }
        
    fn name(&self) -> &'static str { DashboardSettings::key() }

    fn reconfigure(&mut self, config: &DiscordConfig) {
        let new_settings = config.get_module_config::<DashboardSettings>().unwrap_or_default();        
        if new_settings.auto_spawn_dash && !self.settings.auto_spawn_dash {
            self.needs_refresh = true;
        }

        self.settings = new_settings;
    }

    async fn on_event(&mut self, event: &GameEvent, _http: &Arc<Http>, _channel: ChannelId) -> Vec<BotResponse> {
        match event {
            GameEvent::GameCommandEvent(cmd) => {
                crate::auto_dispatch!(self, cmd, [
                    cmd_cta,
                    cmd_playerlist,
                    cmd_dash
                ])
            }
            GameEvent::JoinEvent(_) => {
                self.player_count += 1;
                if self.message_id.is_some() {
                    self.needs_refresh = true;
                }
            }
            GameEvent::ServerStatusEvent(new_status) => {
                self.status = Some(new_status.clone());
            }
            GameEvent::MapChangeEvent(e) => {
                self.current_map = e.new_map.clone();
                if self.message_id.is_some() {
                    self.needs_refresh = true;
                }
            }
            _ => {
                // We'd need a LeaveEvent in notifications.rs for this
                if event.event_type() == "LeaveEvent" {
                    self.player_count = self.player_count.saturating_sub(1);
                    if self.message_id.is_some() {
                        self.needs_refresh = true;
                    }
                }
            }
        }

        NO_RESP // The dashboard doesn't send "new" messages, it edits an existing one
    }

    async fn on_tick(&mut self, http: &Arc<Http>, channel: ChannelId) -> Vec<BotResponse> {
        // Only refresh every 30 seconds or if a major event happened
        if !self.needs_refresh && self.message_id.is_none() {
            return NO_RESP;
        }
        
        if !self.needs_refresh && self.last_update.elapsed() < Duration::from_secs(30) {
            return NO_RESP;
        }

        if self.status.is_none() {
            return NO_RESP;
        }

        let embed = self.build_embed();

        match self.message_id {
            Some(id) => {
                // Edit existing message
                let _ = channel.edit_message(http, id, EditMessage::new().add_embed(embed)).await;
            }
            None => {
                // Create the initial dashboard message
                if let Ok(msg) = channel.send_message(http, CreateMessage::new().add_embed(embed)).await {
                    self.message_id = Some(msg.id);
                }
            }
        }

        self.last_update = Instant::now();
        self.needs_refresh = false;
        NO_RESP
    }
}