
use crate::commands::Command;
use crate::events::models::{CommandRequest, CommandSource, PermissionFlags};
use crate::game::chivalry2::{ATBLGameMode, PlayerFlags};
use crate::game::engine::UWorld;
use crate::resolvers::etc_hooks::o_GetTBLGameMode;
use crate::{serror, sinfo, swarn};
use async_trait::async_trait;
use clap::Parser;
use itertools::Itertools;

#[derive(Parser, Clone)]
#[command(name = "game-info", about = "show game info")]
pub struct GameInfoArgs {}

pub struct GameInfoCommand;

#[async_trait]
impl Command<GameInfoArgs> for GameInfoCommand {
    fn required_permissions(&self) -> PermissionFlags { PermissionFlags::ADMIN }
    fn required_source(&self) -> Option<CommandSource> { Some(CommandSource::ServerConsole) }
    async fn execute(&self, _args: GameInfoArgs, _command: &CommandRequest) {
        use crate::{resolvers::unchained_integration::run_on_game_thread};

        run_on_game_thread(move || {
            if let Some(world) = crate::globals().world() {
                let game_ptr: *mut ATBLGameMode = CALL_ORIGINAL!(GetTBLGameMode(world));
                let uworld_ptr = world as *mut UWorld;

                let game = unsafe { game_ptr.as_mut() };
                if let Some(g) = &game {
                    log_game_info(g);
                }

                match unsafe { (game, uworld_ptr.as_mut().and_then(|w| w.game_state.as_mut())) } {
                    (Some(_), Some(game_state)) => {
                        for player_raw in game_state.player_array.as_mut_slice() {
                            let player_state = match unsafe { (player_raw).as_mut() } {
                                Some(ps) => ps,
                                None => {
                                    swarn!(f; "PlayerState was null");
                                    continue;
                                }
                            };
                            let pname = player_state.base.player_name_private.to_string();
                            sinfo!(f; "{}", pname);
                            let mut flags = "".to_string();
                            if player_state.base.player_flags.contains(PlayerFlags::IS_SPECTATOR) {
                                flags.push_str("spectator|");
                            }
                            if player_state.base.player_flags.contains(PlayerFlags::ONLY_SPECTATOR) {
                                flags.push_str("onlyspectator|");
                            }
                            if player_state.base.player_flags.contains(PlayerFlags::IS_A_BOT) {
                                flags.push_str("bot|");
                            }

                            sinfo!(f; "{}({:?}){}|{}, score: {}, K:{}, A:{}, D:{}, IP:{}",
                                flags,
                                player_state.base.player_flags,
                                player_state.base.player_id,
                                player_state.base.player_name_private,
                                player_state.player_score,
                                player_state.kills,
                                player_state.assists,
                                player_state.deaths,
                                player_state.base.saved_network_address,
                            );
                        }
                    },
                    (g, gs) => {
                        if g.is_none() { serror!(f; "GameMode was null"); }
                        if gs.is_none() { serror!(f; "GameState was null"); }
                    }
                }
            } else {
                crate::swarn!(f; "game-info: globals.world was None; skipping game info collection");
            }
        });
    }
}

fn log_game_info(game: &ATBLGameMode) {
    sinfo!(f; "Name:{}", game.server_name);
    let maplist = game.maplist.as_slice().iter().join(", ");
    sinfo!(f; "MapList:{}", maplist);
    sinfo!(f; "Idle disconnect:{}", game.idle_kick_timer_disconnect);
    sinfo!(f; "Idle spectate:{}", game.idle_kick_timer_spectate);
}
