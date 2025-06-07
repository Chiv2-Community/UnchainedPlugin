use std::{
    io::{BufRead, BufReader},
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
};

use once_cell::sync::Lazy;

fn get_rcon_port() -> Option<u16> {
    Some(9001)
}
pub static COMMAND_PENDING: Lazy<Arc<Mutex<Option<bool>>>>   = Lazy::new(|| Arc::new(Mutex::new(None)));
pub static LAST_COMMAND: Lazy<Arc<Mutex<Option<String>>>> = Lazy::new(|| Arc::new(Mutex::new(None)));

pub fn handle_rcon() {
    let port = match get_rcon_port() {
        Some(p) => p,
        None => return,
    };

    let listener = TcpListener::bind(("127.0.0.1", port))
        .expect("[Rust RCON]: Failed to bind to port");

    println!("[Rust RCON]: Listening on 127.0.0.1:{}", port);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let cmd_store = Arc::clone(&LAST_COMMAND);
                let cmd_pending = Arc::clone(&COMMAND_PENDING);
                thread::spawn(move || {
                    let reader = BufReader::new(stream);
                    for line in reader.lines().flatten() {
                        if !line.trim().is_empty() {
                            println!("[Rust RCON]: Received: {}", line.trim());
                            *cmd_store.lock().unwrap() = Some(line.trim().to_string());
                            *cmd_pending.lock().unwrap() = Some(true);
                        }
                    }
                });
            }
            Err(e) => eprintln!("[Rust RCON]: Connection failed: {}", e),
        }
    }
}
