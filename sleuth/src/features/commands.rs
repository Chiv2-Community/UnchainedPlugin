use std::collections::VecDeque;
use std::sync::Mutex;
use once_cell::sync::Lazy;


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

// TODO: Implement proper interaction with logger, so that the prompt stays in place

#[cfg(feature="cli_commands")]
pub fn spawn_cli_handler() {
    std::thread::spawn(move || {
        let input_source = std::io::stdin();
        let mut buffer = String::new();
        
        while input_source.read_line(&mut buffer).is_ok() {
            let trimmed = buffer.trim().to_string();
            if !trimmed.is_empty() {
                COMMAND_QUEUE.lock().unwrap().push(trimmed);
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