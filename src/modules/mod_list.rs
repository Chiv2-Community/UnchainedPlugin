
use crate::commands::Command;
use crate::events::models::{CommandRequest, CommandSource, PermissionFlags};
use crate::tools::hook_globals::globals;
use crate::sinfo;
use async_trait::async_trait;
use clap::Parser;
use std::sync::mpsc;

#[derive(Parser, Clone)]
#[command(name = "mod-list", about = "Lists all available and active mods")]
pub struct ListModsArgs {}

pub struct ListModsCommand;

#[async_trait]
impl Command<ListModsArgs> for ListModsCommand {
    fn required_permissions(&self) -> PermissionFlags { PermissionFlags::ADMIN }
    fn required_source(&self) -> Option<CommandSource> { Some(CommandSource::ServerConsole) }
    async fn execute(&self, _args: ListModsArgs, _command: &CommandRequest) {
        use crate::{resolvers::unchained_integration::run_on_game_thread};
        let mm_lock = || globals().mod_manager.lock().unwrap();

        if mm_lock().as_ref().is_some_and(|mm| mm.get_available().is_empty()) {
            let (tx, rx) = mpsc::channel();
            sinfo!(f; "Starting scan!");

            run_on_game_thread(move || {
                if let Some(mm) = mm_lock().as_ref() {
                    mm.scan_asset_registry();
                    let _ = tx.send(());
                }
            });
            let _ = rx.recv();
        } else {
            if let Some(mm) = mm_lock().as_ref() {
                sinfo!(f; "Getting list!");
                let _ = mm.scan_active_mod_actors();
            }
        }

        if let Some(mm) = mm_lock().as_ref() {
            mm.dump_to_console();
        }
    }
}
