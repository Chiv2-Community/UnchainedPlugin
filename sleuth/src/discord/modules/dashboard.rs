use crate::discord::config::DiscordConfig;
use crate::discord::config::ModuleConfig;
use crate::discord::core::*;
use crate::discord::responses::*;
use crate::discord::notifications::*;
use crate::sinfo;
use rand::seq::IndexedRandom;
use serde::Deserialize;
use serde::Serialize;
use serenity::all::{Http, ChannelId, CreateMessage, MessageId, CreateEmbed, EditMessage};
use sleuth_macros::handler_command;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Instant, Duration};

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
    ctx: crate::discord::Ctx,
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
            ctx,
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

        let mod_list = if cur_status.active_mods.is_empty() {
            "None".to_string()
        } else {
            cur_status.active_mods
                .iter()
                .map(|m| format!("{} *({})*", m.name, m.version))
                .collect::<Vec<_>>()
                .join("\n- ")
        };

        let all_mod_list = if cur_status.mods.is_empty() {
            "None".to_string()
        } else {
            let list = cur_status.mods
                .iter()
                .filter(|m| !active_names.contains(&m.name))
                .map(|m| format!("{} *({})*", m.name, m.version))
                .collect::<Vec<_>>()
                .join(", ");

            if list.is_empty() { "None".to_string() } else { format!("-# {list}") }
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
    
    #[handler_command("cta", desc="Issue a Call to Arms on the discord server")]
    pub fn cmd_cta(&mut self, message: String, cmd: &GameCommandEvent) -> Vec<BotResponse> {
        if self.status.as_ref().is_none() {
            return NO_RESP;
        }
        let cur_status = self.status.clone().expect("No status available");
        let mod_list = if cur_status.active_mods.is_empty() {
            "None".to_string()
        } else {
            format!("- {}", cur_status.active_mods
                .iter()
                .map(|m| format!("**{}** *({})*", m.name, m.version))
                .collect::<Vec<_>>()
                .join("\n- "))
        };
        
        let sender = &cmd.actor.display_name;

        let templates = [
            // The Classic
            format!("**{}** has issued a __**Call to Arms**__!\nJoin the server and fight for your honor!\nMessage: _{}_", sender, message),
            format!("⚠️ **REINFORCEMENTS NEEDED!**\n**{}** is requesting immediate backup.\nOrders: _{}_", sender, message),
            format!("📢 **BANNERS RAISED!**\n**{}** has sounded the war horn! Rally to their side!\nWar Cry: _{}_", sender, message),                    
            format!("🔥 **TO THE FRONT LINES!**\n**{}** says: _{}_\nDon't let them stand alone!", sender, message),
            format!("⚔️ **{}** is calling for all able-bodied warriors!\n> _{}_", sender, message),
            format!("🍖 **FRESH MEAT!**\n**{}** is getting beat up and needs someone to hide behind. Join now!\nExcuse: *\"{}\"*", sender, message),
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
        
        let mut embed = CreateEmbed::new()
            .title("⚔️ CALL TO ARMS ⚔️")
            .color(chosen_color)
            .description(chosen_description)
            .field("Server", cur_status.name, false)
            .field("Description", cur_status.description, false)
            .field("Current Map", cur_status.current_map, true)
            .field("All Mods", mod_list, true);
        return BotResponse::from(embed).to_main().into_responses(); // FIXME: also write to general
    }
    
    #[handler_command("dash", desc="Display server dashboard (if available)")]
    pub fn cmd_dash(&mut self, cmd: &GameCommandEvent) -> Vec<BotResponse> {
        if cmd.source != CommandSource::Discord || cmd.name != "dash" { return NO_RESP; }
        
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

}

#[async_trait::async_trait]
impl DiscordSubscriber for Dashboard {
    fn get_commands(&self) -> Vec<CommandInfo> {
        crate::auto_help!(self, [
            cmd_cta,
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

    async fn on_event(&mut self, event: &dyn GameEvent, _http: &Arc<Http>, _channel: ChannelId) -> Vec<BotResponse> {
        
        if let Some(cmd) = event.as_any().downcast_ref::<GameCommandEvent>() {
            crate::auto_dispatch!(self, cmd, [
                cmd_cta,
                cmd_dash
            ]);
        }

        let any = event.as_any();

        // crate::sinfo!["Got Event {:#?}", event];
        // Update state based on events
        if let Some(_e) = any.downcast_ref::<JoinEvent>() {
            self.player_count += 1;
            if self.message_id.is_some() {
                self.needs_refresh = true;
            }
        }
        
        if let Some(new_status) = any.downcast_ref::<ServerStatus>() {
            self.status = Some(new_status.clone());
        }

        // We'd need a LeaveEvent in notifications.rs for this
        if event.event_type() == "LeaveEvent" {
            self.player_count = self.player_count.saturating_sub(1);
            if self.message_id.is_some() {
                self.needs_refresh = true;
            }
        }

        if let Some(e) = any.downcast_ref::<MapChangeEvent>() {
            self.current_map = e.new_map.clone();
            if self.message_id.is_some() {
                self.needs_refresh = true;
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
                let _ = channel.edit_message(http, self.message_id.unwrap(), EditMessage::new().add_embed(embed)).await;
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