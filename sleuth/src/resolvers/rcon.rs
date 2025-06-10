use std::{
    io::{stdin, BufRead, BufReader},
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
};

use log::{error, info, warn};
use once_cell::sync::Lazy;


#[cfg(feature="rcon")]
fn get_rcon_port() -> Option<u16> {
    Some(9001)
}
pub static COMMAND_PENDING: Lazy<Arc<Mutex<Option<bool>>>>   = Lazy::new(|| Arc::new(Mutex::new(None)));
pub static LAST_COMMAND: Lazy<Arc<Mutex<Option<String>>>> = Lazy::new(|| Arc::new(Mutex::new(None)));
// pub static FLAST_COMMAND: Lazy<Arc<Mutex<Option<FString>>>> = Lazy::new(|| Arc::new(Mutex::new(None)));

#[cfg(feature="rcon")]
pub fn handle_rcon() {
    let port = match get_rcon_port() {
        Some(p) => p,
        None => return,
    };

    let listener = TcpListener::bind(("127.0.0.1", port))
        .expect("[RCON] Failed to bind to port");

    info!("[RCON] Listening on 127.0.0.1:{port}");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let cmd_store: Arc<Mutex<Option<String>>> = Arc::clone(&LAST_COMMAND);
                let cmd_pending = Arc::clone(&COMMAND_PENDING);
                thread::spawn(move || {
                    let reader = BufReader::new(stream);
                    for line in reader.lines().flatten() {
                        if !line.trim().is_empty() {
                            warn!("[RCON] Received: {}", line.trim());
                            *cmd_store.lock().unwrap() = Some(line.trim().to_string());
                            *cmd_pending.lock().unwrap() = Some(true);
                        }
                    }
                });
            }
            Err(e) => error!("[RCON] Connection failed: {e}"),
        }
    }
}


// FIME: Nihi: this need some validation
// maybe a proper prompt etc
#[cfg(feature="cli-commands")]
pub fn handle_cmd() {
    // let line = String::new();
    loop {
        let mut input = String::new();
        stdin().read_line(&mut input)
            .expect("UTF-8 unsupported");
        let cmd_store: Arc<Mutex<Option<String>>> = Arc::clone(&LAST_COMMAND);
        *cmd_store.lock().unwrap() = Some(input.trim().to_string());
        match input.as_str() {
            "findobj" => {
                crate::sdebug!(f; "findobj {:?}", 123);
            }
            _ => {},
        }
    }
}