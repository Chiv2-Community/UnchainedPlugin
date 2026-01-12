use std::{
    io::{BufRead, BufReader}, net::TcpListener, thread
};
use crate::{tools::hook_globals::globals};


#[cfg(feature="rcon_commands")]
pub fn handle_rcon() {
    use crate::tools::hook_globals::cli_args;

    let port = match cli_args().rcon_port {
        Some(p) => p,
        None => return,
    };

    let listener = TcpListener::bind(("127.0.0.1", port))
        .expect("[RCON] Failed to bind to port");

    log::info!(target: "RCON", "RCON Listening on 127.0.0.1:{port}");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(move || {
                    let reader = BufReader::new(stream);
                    for line in reader.lines().map_while(Result::ok) {
                        if !line.trim().is_empty() {
                            use crate::commands::NATIVE_COMMAND_QUEUE;

                            log::warn!(target: "RCON", "RCON Received: {}", line.trim());
                            NATIVE_COMMAND_QUEUE.lock().unwrap().push(line.trim().to_string());
                        }
                    }
                });
            }
            Err(e) => log::error!(target: "RCON", "RCON Connection failed: {e}"),
        }
    }
}