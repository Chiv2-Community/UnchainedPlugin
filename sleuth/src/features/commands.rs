use std::collections::VecDeque;
use std::sync::Mutex;
use once_cell::sync::Lazy;
use inventory;

/// The struct used to register a new command
pub struct ConsoleCommandHandler {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub description: &'static str,
    /// mapping for parameter help (e.g., "id" => "The player ID")
    pub field_description: &'static [(&'static str, &'static str)],
    /// takes the raw arguments (excluding command name)
    pub handler: fn(args: Vec<String>),
}

inventory::collect!(ConsoleCommandHandler);

fn handle_help(_args: Vec<String>) {
    println!("\n--- Available Console Commands ---");
    
    for cmd in inventory::iter::<ConsoleCommandHandler> {
        let aliases = if cmd.aliases.is_empty() {
            "".to_string()
        } else {
            format!(" (Aliases: {})", cmd.aliases.join(", "))
        };

        println!("> {}{}", cmd.name.to_uppercase(), aliases);
        println!("  Description: {}", cmd.description);

        if !cmd.field_description.is_empty() {
            println!("  Parameters:");
            for (field, desc) in cmd.field_description {
                println!("    - {:<10} : {}", field, desc);
            }
        }
        println!("----------------------------------");
    }
}

inventory::submit! {
    ConsoleCommandHandler {
        name: "help",
        aliases: &["?", "commands"],
        description: "Displays this help menu.",
        field_description: &[],
        handler: handle_help,
    }
}


pub static COMMAND_QUEUE: Lazy<Mutex<CommandManager>> = Lazy::new(|| {
    Mutex::new(CommandManager::new())
});

pub struct CommandManager {
    queue: VecDeque<String>,
}

impl CommandManager {
    fn new() -> Self {
        Self { queue: VecDeque::new() }
    }

    /// Add a command to the queue (Thread-safe via the Mutex above)
    pub fn push(&mut self, cmd: String) {
        if !cmd.is_empty() {
            self.queue.push_back(cmd);
        }
    }

    /// Pop the next command (Used by your Game Thread hook)
    pub fn pop(&mut self) -> Option<String> {
        self.queue.pop_front()
    }
}

pub fn dispatch_command(input: &str) -> bool {
    let parts: Vec<String> = input.split_whitespace().map(|s| s.to_string()).collect();
    if parts.is_empty() { return false; }

    let cmd_name = parts[0].to_lowercase();
    let args = parts[1..].to_vec();

    for cmd in inventory::iter::<ConsoleCommandHandler> {
        if cmd.name == cmd_name || cmd.aliases.contains(&cmd_name.as_str()) {
            (cmd.handler)(args);
            return true;
        }
    }
    // println!("Unknown command: {}. Type 'help' for a list.", cmd_name);
    false
}

#[macro_export]
macro_rules! CREATE_COMMAND {
    (
        $name:expr, 
        [$($alias:expr),*], 
        $desc:expr, 
        {$($f_name:expr => $f_desc:expr),*}, 
        |$args:ident| $body:block
    ) => {
        paste::paste! {
            #[allow(unused_variables)]
            fn [< __cmd_handler_ $name >] ($args: Vec<String>) $body

            inventory::submit! {
                ConsoleCommandHandler {
                    name: $name,
                    aliases: &[$($alias),*],
                    description: $desc,
                    field_description: &[ $(($f_name, $f_desc)),* ],
                    handler: [< __cmd_handler_ $name >],
                }
            }
        }
    };
}

// TODO: Implement proper interaction with logger, so that the prompt stays in place
#[cfg(feature="cli_commands")]
pub fn spawn_cli_handler() {
    std::thread::spawn(move || {
        let input_source = std::io::stdin();
        let mut buffer = String::new();
        
        while input_source.read_line(&mut buffer).is_ok() {
            let trimmed = buffer.trim().to_string();
            if !trimmed.is_empty() {
                // run commands in game thread
                // TODO: yes it might cause a small hitch when executing             
                COMMAND_QUEUE.lock().unwrap().push(trimmed);
                // match dispatch_command(trimmed.as_str()) {
                //     true => crate::sinfo!(f; "Executed custom command"),
                //     false => COMMAND_QUEUE.lock().unwrap().push(trimmed)
                // };                
            }
            buffer.clear();
        }
    });
}

// use log::{Level, Metadata, Record, Log};
// use log4rs::append::Append;
// use std::sync::mpsc::Sender;

// #[derive(Debug)]
// pub struct ChannelAppender {
//     sender: Sender<String>,
// }

// impl Append for ChannelAppender {
//     fn append(&self, record: &Record) -> anyhow::Result<()> {
//         // Format the message however you like
//         let msg = format!("[{}] {}", record.level(), record.args());
//         let _ = self.sender.send(msg);
//         Ok(())
//     }

//     fn flush(&self) {}
// }

// pub fn init_logging(tx: Sender<String>) {
//     let appender = ChannelAppender { sender: tx };

//     let config = log4rs::Config::builder()
//         .appender(log4rs::config::Appender::builder().build("channel", Box::new(appender)))
//         .build(log4rs::config::Root::builder().appender("channel").build(log::LevelFilter::Info))
//         .unwrap();

//     log4rs::init_config(config).unwrap();
// }

// pub fn spawn_ui_thread() {
//     let (tx, rx) = std::sync::mpsc::channel();
//     init_logging(tx); // Pass the sender to log4rs

//     std::thread::spawn(move || {
//         let mut rl = rustyline::DefaultEditor::new().unwrap();
        
//         loop {
//             // Non-blocking log check
//             while let Ok(log_msg) = rx.try_recv() {
//                 // If using Rustyline, 'external_print' is the cleanest way
//                 // to print without breaking the user's current typing.
//                 println!("\r{}", log_msg); 
//             }

//             // Blocking input check (with a small timeout if you want to be fancy)
//             if let Ok(line) = rl.readline("unchained >> ") {
//                 let _ = rl.add_history_entry(&line);
//                 COMMAND_QUEUE.lock().unwrap().push(line);
//             }
//         }
//     });
// }