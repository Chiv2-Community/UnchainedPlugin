use snow::{Builder, HandshakeState, TransportState};
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use zeroize::Zeroizing;

const PROTOCOL: &str = "Noise_NNpsk0_25519_AESGCM_SHA256";
const PROLOGUE: &[u8] = b"Unchained RCON v1";
pub const MAX_COMMAND_BYTES: usize = 4096;
const TAG_BYTES: usize = 16;
const MAX_HANDSHAKE_BYTES: usize = 128;

pub fn load_psk(path: &Path) -> io::Result<Zeroizing<[u8; 32]>> {
    let mut encoded = Zeroizing::new(Vec::new());
    File::open(path)?.take(129).read_to_end(&mut encoded)?;
    if encoded.len() > 128 {
        return Err(invalid("PSK file is too large"));
    }
    parse_psk(&encoded)
}

fn parse_psk(encoded: &[u8]) -> io::Result<Zeroizing<[u8; 32]>> {
    let text = std::str::from_utf8(encoded)
        .map_err(|_| invalid("PSK must contain 64 hexadecimal characters"))?
        .trim();
    if text.len() != 64 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid("PSK must contain 64 hexadecimal characters"));
    }
    let mut key = Zeroizing::new([0; 32]);
    for (i, byte) in key.iter_mut().enumerate() {
        *byte =
            u8::from_str_radix(&text[2 * i..2 * i + 2], 16).map_err(|_| invalid("Invalid PSK"))?;
    }
    Ok(key)
}

pub struct Session {
    state: TransportState,
}

impl Session {
    pub fn accept(stream: &mut (impl Read + Write), psk: &[u8; 32]) -> io::Result<Self> {
        let mut state = handshake(psk, false)?;
        receive_handshake(stream, &mut state)?;
        send_handshake(stream, &mut state)?;
        Self::finish(state)
    }

    pub fn connect(stream: &mut (impl Read + Write), psk: &[u8; 32]) -> io::Result<Self> {
        let mut state = handshake(psk, true)?;
        send_handshake(stream, &mut state)?;
        receive_handshake(stream, &mut state)?;
        Self::finish(state)
    }

    fn finish(state: HandshakeState) -> io::Result<Self> {
        let state = state.into_transport_mode().map_err(crypto_error)?;
        Ok(Self { state })
    }

    pub fn write_command(&mut self, writer: &mut impl Write, command: &str) -> io::Result<()> {
        validate_command(command)?;
        let mut frame = vec![0; command.len() + TAG_BYTES];
        let size = self
            .state
            .write_message(command.as_bytes(), &mut frame)
            .map_err(crypto_error)?;
        write_frame(writer, &frame[..size])
    }

    pub fn read_command(&mut self, reader: &mut impl Read) -> io::Result<Option<String>> {
        let Some(frame) = read_frame(reader, MAX_COMMAND_BYTES + TAG_BYTES)? else {
            return Ok(None);
        };
        let mut plaintext = Zeroizing::new(vec![0; frame.len()]);
        let size = self
            .state
            .read_message(&frame, &mut plaintext)
            .map_err(crypto_error)?;
        let command =
            std::str::from_utf8(&plaintext[..size]).map_err(|_| invalid("Command is not UTF-8"))?;
        validate_command(command)?;
        Ok(Some(command.to_owned()))
    }
}

fn handshake(psk: &[u8; 32], initiator: bool) -> io::Result<HandshakeState> {
    let params = PROTOCOL.parse().map_err(crypto_error)?;
    let builder = Builder::new(params)
        .prologue(PROLOGUE)
        .map_err(crypto_error)?
        .psk(0, psk)
        .map_err(crypto_error)?;
    if initiator {
        builder.build_initiator().map_err(crypto_error)
    } else {
        builder.build_responder().map_err(crypto_error)
    }
}

fn send_handshake(writer: &mut impl Write, state: &mut HandshakeState) -> io::Result<()> {
    let mut frame = [0; MAX_HANDSHAKE_BYTES];
    let size = state.write_message(&[], &mut frame).map_err(crypto_error)?;
    write_frame(writer, &frame[..size])
}

fn receive_handshake(reader: &mut impl Read, state: &mut HandshakeState) -> io::Result<()> {
    let frame = read_frame(reader, MAX_HANDSHAKE_BYTES)?
        .ok_or_else(|| io::Error::from(io::ErrorKind::UnexpectedEof))?;
    let mut payload = [0; MAX_HANDSHAKE_BYTES];
    if state
        .read_message(&frame, &mut payload)
        .map_err(crypto_error)?
        != 0
    {
        return Err(invalid("Unexpected handshake payload"));
    }
    Ok(())
}

fn read_frame(reader: &mut impl Read, limit: usize) -> io::Result<Option<Vec<u8>>> {
    let mut header = [0; 2];
    loop {
        match reader.read(&mut header[..1]) {
            Ok(0) => return Ok(None),
            Ok(_) => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    reader.read_exact(&mut header[1..])?;
    let size = u16::from_be_bytes(header) as usize;
    if !(TAG_BYTES..=limit).contains(&size) {
        return Err(invalid("Invalid RCON frame size"));
    }
    let mut frame = vec![0; size];
    reader.read_exact(&mut frame)?;
    Ok(Some(frame))
}

fn write_frame(writer: &mut impl Write, frame: &[u8]) -> io::Result<()> {
    let size = u16::try_from(frame.len()).map_err(|_| invalid("RCON frame too large"))?;
    writer.write_all(&size.to_be_bytes())?;
    writer.write_all(frame)?;
    writer.flush()
}

fn validate_command(command: &str) -> io::Result<()> {
    if command.len() > MAX_COMMAND_BYTES
        || command.chars().any(|ch| matches!(ch, '\r' | '\n' | '\0'))
    {
        return Err(invalid(
            "Command must be one line of at most 4096 UTF-8 bytes without NUL",
        ));
    }
    Ok(())
}

fn crypto_error(_: snow::Error) -> io::Error {
    invalid("RCON authentication or protocol failure")
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::net::{TcpListener, TcpStream};
    use std::thread;
    use std::time::Duration;

    fn sessions_with_keys(
        client_key: [u8; 32],
        server_key: [u8; 32],
    ) -> (io::Result<Session>, io::Result<Session>) {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            Session::accept(&mut stream, &server_key)
        });
        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let client = Session::connect(&mut stream, &client_key);
        (client, server.join().unwrap())
    }

    fn sessions() -> (Session, Session) {
        let (client, server) = sessions_with_keys([7; 32], [7; 32]);
        (client.unwrap(), server.unwrap())
    }

    fn encode(client: &mut Session, command: &str) -> Vec<u8> {
        let mut frame = Vec::new();
        client.write_command(&mut frame, command).unwrap();
        frame
    }

    #[test]
    fn psk_requires_exactly_32_hex_encoded_bytes() {
        assert_eq!(*parse_psk("aB".repeat(32).as_bytes()).unwrap(), [0xab; 32]);
        assert_eq!(
            *parse_psk(format!("{}\n", "01".repeat(32)).as_bytes()).unwrap(),
            [1; 32]
        );
        for encoded in [
            String::new(),
            "a".repeat(63),
            "a".repeat(65),
            "g".repeat(64),
            "🩸".repeat(16),
        ] {
            assert!(parse_psk(encoded.as_bytes()).is_err());
        }
    }

    #[test]
    fn encrypted_commands_round_trip_in_order_without_plaintext_on_wire() {
        let (mut client, mut server) = sessions();
        let commands = ["say hello 🩸", "changemap FFA_Wardenglade", ""];
        let mut wire = Vec::new();
        for command in commands {
            client.write_command(&mut wire, command).unwrap();
        }
        for command in commands.iter().filter(|command| !command.is_empty()) {
            assert!(!wire
                .windows(command.len())
                .any(|bytes| bytes == command.as_bytes()));
        }
        let mut reader = Cursor::new(wire);
        for command in commands {
            assert_eq!(
                server.read_command(&mut reader).unwrap().as_deref(),
                Some(command)
            );
        }
        assert!(server.read_command(&mut reader).unwrap().is_none());
    }

    #[test]
    fn wrong_psk_cannot_complete_handshake() {
        let (client, server) = sessions_with_keys([1; 32], [2; 32]);
        assert!(client.is_err());
        assert!(server.is_err());
    }

    #[test]
    fn plaintext_is_rejected_before_any_command_can_be_read() {
        assert!(Session::accept(&mut Cursor::new(b"say hello\n".to_vec()), &[7; 32]).is_err());
        let (_, mut server) = sessions();
        assert!(server
            .read_command(&mut Cursor::new(b"say hello\n"))
            .is_err());
    }

    #[test]
    fn tampered_commands_are_rejected() {
        let (mut client, mut server) = sessions();
        let mut frame = encode(&mut client, "say authenticated");
        *frame.last_mut().unwrap() ^= 1;
        assert!(server.read_command(&mut Cursor::new(frame)).is_err());
    }

    #[test]
    fn replayed_commands_are_rejected_in_the_same_session() {
        let (mut client, mut server) = sessions();
        let frame = encode(&mut client, "say once");
        assert!(server
            .read_command(&mut Cursor::new(&frame))
            .unwrap()
            .is_some());
        assert!(server.read_command(&mut Cursor::new(&frame)).is_err());
    }

    #[test]
    fn reordered_commands_are_rejected() {
        let (mut client, mut server) = sessions();
        let _first = encode(&mut client, "say first");
        let second = encode(&mut client, "say second");
        assert!(server.read_command(&mut Cursor::new(second)).is_err());
    }

    #[test]
    fn commands_cannot_be_replayed_across_sessions_or_directions() {
        let (mut client, _) = sessions();
        let (_, mut other_server) = sessions();
        let frame = encode(&mut client, "say bound to session");
        assert!(other_server.read_command(&mut Cursor::new(&frame)).is_err());
        assert!(client.read_command(&mut Cursor::new(&frame)).is_err());
    }

    #[test]
    fn replaying_a_handshake_does_not_allow_replaying_its_commands() {
        let mut initiator = handshake(&[7; 32], true).unwrap();
        let mut initial = Vec::new();
        send_handshake(&mut initial, &mut initiator).unwrap();
        let mut original = handshake(&[7; 32], false).unwrap();
        receive_handshake(&mut Cursor::new(&initial), &mut original).unwrap();
        let mut response = Vec::new();
        send_handshake(&mut response, &mut original).unwrap();
        receive_handshake(&mut Cursor::new(response), &mut initiator).unwrap();
        let mut client = Session::finish(initiator).unwrap();
        let frame = encode(&mut client, "say original session");

        let mut replayed = handshake(&[7; 32], false).unwrap();
        receive_handshake(&mut Cursor::new(initial), &mut replayed).unwrap();
        send_handshake(&mut Vec::new(), &mut replayed).unwrap();
        let mut server = Session::finish(replayed).unwrap();
        assert!(server.read_command(&mut Cursor::new(frame)).is_err());
    }

    #[test]
    fn oversized_frames_are_rejected_before_reading_the_body() {
        let (_, mut server) = sessions();
        let error = server
            .read_command(&mut Cursor::new(u16::MAX.to_be_bytes()))
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn truncated_frames_are_errors_not_clean_disconnects() {
        for bytes in [vec![0], vec![0, 16, 1, 2, 3]] {
            let (_, mut server) = sessions();
            let error = server.read_command(&mut Cursor::new(bytes)).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
        }
    }

    #[test]
    fn command_size_limit_counts_utf8_bytes() {
        let (mut client, mut server) = sessions();
        let command = "🩸".repeat(MAX_COMMAND_BYTES / 4);
        let frame = encode(&mut client, &command);
        assert_eq!(
            server.read_command(&mut Cursor::new(frame)).unwrap(),
            Some(command)
        );
        assert!(client
            .write_command(&mut Vec::new(), &"a".repeat(MAX_COMMAND_BYTES + 1))
            .is_err());
    }

    #[test]
    fn invalid_utf8_and_multiline_commands_are_rejected_after_authentication() {
        for command in [
            b"\xff".as_slice(),
            b"say first\nsay second",
            b"say\rcommand",
            b"say\0command",
        ] {
            let (mut client, mut server) = sessions();
            let mut ciphertext = vec![0; command.len() + TAG_BYTES];
            let size = client
                .state
                .write_message(command, &mut ciphertext)
                .unwrap();
            let mut wire = Vec::new();
            write_frame(&mut wire, &ciphertext[..size]).unwrap();
            assert!(server.read_command(&mut Cursor::new(wire)).is_err());
        }
    }

    #[test]
    fn client_rejects_invalid_commands_without_advancing_the_session() {
        let (mut client, mut server) = sessions();
        for command in ["say\ncommand", "say\rcommand", "say\0command"] {
            let mut wire = Vec::new();
            assert!(client.write_command(&mut wire, command).is_err());
            assert!(wire.is_empty());
        }
        let frame = encode(&mut client, "say valid");
        assert_eq!(
            server
                .read_command(&mut Cursor::new(frame))
                .unwrap()
                .as_deref(),
            Some("say valid")
        );
    }

    #[test]
    fn frame_reader_handles_fragmented_streams() {
        struct Fragments(Cursor<Vec<u8>>);
        impl Read for Fragments {
            fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
                let size = buffer.len().min(1);
                self.0.read(&mut buffer[..size])
            }
        }
        let (mut client, mut server) = sessions();
        let wire = encode(&mut client, "say fragmented");
        assert_eq!(
            server
                .read_command(&mut Fragments(Cursor::new(wire)))
                .unwrap()
                .as_deref(),
            Some("say fragmented")
        );
    }
}
