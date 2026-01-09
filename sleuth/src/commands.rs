use std::sync::Mutex;

use inventory;
use once_cell::sync::Lazy;
use crate::resolvers::unchained_integration::run_on_game_thread;
use anyhow::{Result, anyhow};

pub type CommandResult = Result<()>;

/// The structure submitted to the inventory registry
pub struct ConsoleCommand {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub subcommand: Option<&'static str>,
    pub description: &'static str,
    pub params: &'static str,        // NEW: Stores parameter info
    pub game_thread_required: bool,
    pub handler: fn(Vec<String>) -> ::anyhow::Result<()>,
}

inventory::collect!(ConsoleCommand);

/// The Dispatcher: Handles parsing, matching, and thread safety
pub fn dispatch_command(input: &str) -> bool {
    let parts: Vec<String> = input.split_whitespace().map(|s| s.to_string()).collect();
    if parts.is_empty() { return false; }

    let cmd_input = &parts[0];
    
    let command = ::inventory::iter::<crate::commands::ConsoleCommand>.into_iter().find(|c| {
        let name_matches = c.name == cmd_input;
        let alias_matches = c.aliases.contains(&cmd_input.as_str());
        
        match c.subcommand {
            Some(sub) => {
                // Case A: Standard 'mod dump' or 'm dump'
                let is_standard = (name_matches || alias_matches) && 
                                parts.len() > 1 && 
                                sub == parts[1];
                
                // Case B: Shortcut 'dmpmod'
                // We only check aliases here so the main 'name' still requires the sub
                let is_shortcut = alias_matches && !is_standard;

                is_standard || is_shortcut
            },
            None => name_matches || alias_matches
        }
    });

    if let Some(cmd) = command {
        let skip_count = if cmd.subcommand.is_some() { 2 } else { 1 };
        let args: Vec<String> = parts.into_iter().skip(skip_count).collect();

        if cmd.game_thread_required {
            let handler = cmd.handler;
            run_on_game_thread(move || {
                if let Err(e) = (handler)(args) {
                    log::error!(target: "Commands", "Custom Command Error: {e}");
                }
            });
        } else {
            if let Err(e) = (cmd.handler)(args) {
                log::error!(target: "Commands", "Custom Command Error: {e}");
            }
        }
        true // We handled it!
    } else {
        false // Not a custom command, let the CLI forward it
    }
}

// Mock of your job queue - Replace with your actual engine hook
// fn push_to_game_thread_queue<F>(f: F) where F: FnOnce() + Send + 'static {
//     // Your engine's actual queue logic goes here
//     f(); 
// }

pub static NATIVE_COMMAND_QUEUE: Lazy<Mutex<Vec<String>>> = Lazy::new(|| {
    Mutex::new(Vec::new())
});

#[cfg(feature="cli_commands")]
pub fn spawn_cli_handler() {
    std::thread::spawn(move || {
        let input_source = std::io::stdin();
        let mut buffer = String::new();
        
        while input_source.read_line(&mut buffer).is_ok() {
            let trimmed = buffer.trim().to_string();
            if !trimmed.is_empty() {
                // 1. Try custom Rust command first
                if !dispatch_command(&trimmed) {
                    // 2. Fallback to engine native command
                    NATIVE_COMMAND_QUEUE.lock().unwrap().push(trimmed);
                }              
            }
            buffer.clear();
        }
    });
}

#[sleuth_macros::command(name = "help", desc = "Lists all commands")]
fn help_command(command_name: Option<String>) -> ::anyhow::Result<()> {
    // 1. Rename variable to 'all_cmds' to avoid 'iter' name collision
    // 2. Collect into Vec so we can iterate multiple times
    let all_cmds: Vec<&crate::commands::ConsoleCommand> = ::inventory::iter::<crate::commands::ConsoleCommand>
        .into_iter()
        .collect();

    if let Some(target) = command_name {
        println!("--- Help for '{}' ---", target);
        let mut found = false;
        for cmd in &all_cmds {
            if cmd.name == target {
                let sub = cmd.subcommand.map(|s| format!(" {}", s)).unwrap_or_default();
                println!("  {}{} {} - {}", cmd.name, sub, cmd.params, cmd.description);
                found = true;
            }
        }
        if !found { println!("No command found named '{}'", target); }
    } else {
        println!("--- Available Commands ---");
        let mut parents = ::std::collections::BTreeSet::new();
        for cmd in &all_cmds {
            parents.insert(cmd.name);
        }

        for parent in parents {
            let subs: Vec<_> = all_cmds.iter()
                .filter(|c| c.name == parent && c.subcommand.is_some())
                .collect();

            if subs.is_empty() {
                if let Some(cmd) = all_cmds.iter().find(|c| c.name == parent) {
                    println!("  {:15} {:25} - {}", cmd.name, cmd.params, cmd.description);
                }
            } else {
                let sub_names: Vec<&str> = subs.iter().map(|c| c.subcommand.unwrap()).collect();
                println!("  {:15} [subs: {}]", parent, sub_names.join(", "));
            }
        }
        println!("\nUse 'help <command>' for more info.");
    }
    Ok(())
}