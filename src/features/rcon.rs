#[cfg(feature = "rcon_commands")]
pub mod transport;

#[cfg(feature = "rcon_commands")]
pub fn handle_rcon() {
    use crate::commands::NATIVE_COMMAND_QUEUE;
    use crate::tools::hook_globals::cli_args;
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};
    use std::thread;

    struct ConnectionSlot(Arc<Mutex<usize>>);
    impl Drop for ConnectionSlot {
        fn drop(&mut self) {
            *self.0.lock().unwrap() -= 1;
        }
    }

    let Some(port) = cli_args().rcon_port else {
        return;
    };
    let Some(path) = cli_args().rcon_psk_file.as_ref() else {
        log::error!(target: "RCON", "RCON disabled: --rcon-psk-file is required");
        return;
    };
    let key = match transport::load_psk(path) {
        Ok(key) => Arc::new(key),
        Err(error) => {
            log::error!(target: "RCON", "RCON disabled: cannot load PSK: {error}");
            return;
        }
    };
    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(listener) => listener,
        Err(error) => {
            log::error!(target: "RCON", "Cannot bind RCON listener: {error}");
            return;
        }
    };
    log::info!(target: "RCON", "Encrypted RCON listening on 127.0.0.1:{port}");

    let active = Arc::new(Mutex::new(0usize));
    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(error) => {
                log::warn!(target: "RCON", "RCON connection failed: {error}");
                continue;
            }
        };
        let mut count = active.lock().unwrap();
        if *count >= 32 {
            continue;
        }
        *count += 1;
        drop(count);

        let key = Arc::clone(&key);
        let slot = ConnectionSlot(Arc::clone(&active));
        thread::spawn(move || {
            let _slot = slot;
            let result = handle_connection(&mut stream, &key, |command| {
                NATIVE_COMMAND_QUEUE.lock().unwrap().push(command);
                log::debug!(target: "RCON", "Authenticated RCON command queued");
            });
            if let Err(error) = result {
                log::debug!(target: "RCON", "RCON connection closed: {error}");
            }
        });
    }
}

#[cfg(feature = "rcon_commands")]
fn handle_connection(
    stream: &mut std::net::TcpStream,
    key: &[u8; 32],
    mut enqueue: impl FnMut(String),
) -> std::io::Result<()> {
    use std::time::Duration;

    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut session = transport::Session::accept(stream, key)?;
    stream.set_read_timeout(Some(Duration::from_secs(300)))?;
    while let Some(command) = session.read_command(stream)? {
        let command = command.trim();
        if !command.is_empty() {
            enqueue(command.to_owned());
        }
    }
    Ok(())
}

#[cfg(all(test, feature = "rcon_commands"))]
mod tests {
    use super::*;
    use std::io::{self, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;
    use std::time::Duration;

    const KEY: [u8; 32] = [7; 32];

    fn dispatched(client: impl FnOnce(&mut TcpStream)) -> (io::Result<()>, Vec<String>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut commands = Vec::new();
            let result = handle_connection(&mut stream, &KEY, |command| commands.push(command));
            (result, commands)
        });
        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        client(&mut stream);
        drop(stream);
        server.join().unwrap()
    }

    fn frame(message: &[u8]) -> Vec<u8> {
        let mut wire = (message.len() as u16).to_be_bytes().to_vec();
        wire.extend_from_slice(message);
        wire
    }

    #[test]
    fn unauthenticated_input_never_reaches_dispatch() {
        for wire in [
            b"say plaintext\n".to_vec(),
            frame(b"say framed plaintext"),
            frame(&[0; 48]),
            vec![0, 48, 1, 2, 3],
        ] {
            let (result, commands) = dispatched(|stream| stream.write_all(&wire).unwrap());
            assert!(result.is_err());
            assert!(commands.is_empty());
        }
    }

    #[test]
    fn wrong_psk_never_reaches_dispatch() {
        let (result, commands) = dispatched(|stream| {
            assert!(transport::Session::connect(stream, &[8; 32]).is_err());
        });
        assert!(result.is_err());
        assert!(commands.is_empty());
    }

    #[test]
    fn handshake_without_psk_is_rejected() {
        let (result, commands) = dispatched(|stream| {
            let mut client = snow::Builder::new("Noise_NN_25519_AESGCM_SHA256".parse().unwrap())
                .prologue(b"Unchained RCON v1")
                .unwrap()
                .build_initiator()
                .unwrap();
            let mut message = [0; 128];
            let size = client.write_message(&[], &mut message).unwrap();
            stream.write_all(&frame(&message[..size])).unwrap();
        });
        assert!(result.is_err());
        assert!(commands.is_empty());
    }

    #[test]
    fn handshake_payload_cannot_dispatch_commands() {
        let (result, commands) = dispatched(|stream| {
            let mut client =
                snow::Builder::new("Noise_NNpsk0_25519_AESGCM_SHA256".parse().unwrap())
                    .prologue(b"Unchained RCON v1")
                    .unwrap()
                    .psk(0, &KEY)
                    .unwrap()
                    .build_initiator()
                    .unwrap();
            let mut message = [0; 128];
            let size = client
                .write_message(b"say handshake injection", &mut message)
                .unwrap();
            stream.write_all(&frame(&message[..size])).unwrap();
        });
        assert!(result.is_err());
        assert!(commands.is_empty());
    }

    #[test]
    fn handshake_alone_never_dispatches_a_command() {
        let (result, commands) = dispatched(|stream| {
            transport::Session::connect(stream, &KEY).unwrap();
        });
        assert!(result.is_ok());
        assert!(commands.is_empty());
    }

    #[test]
    fn authenticated_commands_reach_dispatch_in_order() {
        let (result, commands) = dispatched(|stream| {
            let mut session = transport::Session::connect(stream, &KEY).unwrap();
            for command in ["  say first  ", " ", "say second 🩸"] {
                session.write_command(stream, command).unwrap();
            }
        });
        assert!(result.is_ok());
        assert_eq!(commands, ["say first", "say second 🩸"]);
    }

    #[test]
    fn authentication_failure_stops_dispatch_including_later_valid_frames() {
        let (result, commands) = dispatched(|stream| {
            let mut session = transport::Session::connect(stream, &KEY).unwrap();
            let mut wire = Vec::new();
            session.write_command(&mut wire, "say tampered").unwrap();
            *wire.last_mut().unwrap() ^= 1;
            session
                .write_command(&mut wire, "say must not execute")
                .unwrap();
            stream.write_all(&wire).unwrap();
        });
        assert!(result.is_err());
        assert!(commands.is_empty());
    }

    #[test]
    fn replay_never_dispatches_a_command_twice() {
        let (result, commands) = dispatched(|stream| {
            let mut session = transport::Session::connect(stream, &KEY).unwrap();
            let mut wire = Vec::new();
            session.write_command(&mut wire, "say once").unwrap();
            wire.extend_from_within(..);
            session
                .write_command(&mut wire, "say must not execute")
                .unwrap();
            stream.write_all(&wire).unwrap();
        });
        assert!(result.is_err());
        assert_eq!(commands, ["say once"]);
    }
}
