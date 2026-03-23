use async_trait::async_trait;
use clap::Parser;
use crate::commands::NATIVE_COMMAND_QUEUE;

use crate::commands::Command;
use crate::events::models::{CommandRequest, PermissionFlags};

#[derive(Parser, Clone)]
#[command(name = "cmd", about = "Execute a console command")]
pub struct CmdArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub command: Vec<String>,
}

pub struct CmdCommand;

#[async_trait]
impl Command<CmdArgs> for CmdCommand {
    fn required_permissions(&self) -> PermissionFlags {
        PermissionFlags::ADMIN
    }

    fn group(&self) -> String {
        "Administration".to_string()
    }

    async fn execute(&self, args: CmdArgs, _command: &CommandRequest) {
        let full_command = args.command.join(" ");
        NATIVE_COMMAND_QUEUE.lock().unwrap().push(full_command.clone());
    }
}
