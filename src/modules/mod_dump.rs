
use crate::commands::Command;
use crate::events::models::{CommandRequest, CommandSource, PermissionFlags};
use crate::tools::hook_globals::globals;
use crate::sinfo;
use async_trait::async_trait;
use clap::Parser;

#[derive(Parser, Clone)]
#[command(name = "mod-dump", about = "Dumps mod list. Path is optional")]
pub struct DumpModsArgs {
    pub path: Option<String>,
}

pub struct DumpModsCommand;

#[async_trait]
impl Command<DumpModsArgs> for DumpModsCommand {
    fn group(&self) -> &'static str { "Mod Management" }
    fn required_permissions(&self) -> PermissionFlags { PermissionFlags::ADMIN }
    fn required_source(&self) -> Option<CommandSource> { Some(CommandSource::ServerConsole) }
    async fn execute(&self, args: DumpModsArgs, _command: &CommandRequest) {
        let target: &str = args.path.as_deref().unwrap_or("ingame_mod_registry.json");
        let mm_lock = || globals().mod_manager.lock().unwrap();
        if let Some(mm) = mm_lock().as_ref() {
            let _ = mm.serialize_registry(target);
        }
        sinfo!(f; "Registry saved to \'{}\'", target);
    }
}
