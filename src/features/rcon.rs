use std::{
    io::{BufRead, BufReader}, net::TcpListener, thread
};


#[cfg(feature="rcon_commands")]
pub fn handle_rcon() {
    use crate::tools::hook_globals::cli_args;

    let port = match cli_args().rcon_port {
        Some(p) => p,
        None => return,
    };

    let listener = TcpListener::bind(("127.0.0.1", port))
        .expect("[RCON] Failed to bind to port");

    sinfo!("RCON Listening on 127.0.0.1:{port}");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread::spawn(move || {
                    let reader = BufReader::new(stream);
                    for line in reader.lines().map_while(Result::ok) {
                        let line = line.trim();
                        if line.is_empty() { continue; }

                        swarn!("RCON Received: {}", line);
                        if cli_args().is_server() {
                            if let Some(command) = console_say_request(line) {
                                crate::features::events::EVENT_SYSTEM.game_event_publisher.publish(
                                    crate::events::models::GameEvent::CommandRequestEvent(command));
                                continue;
                            }
                        }

                        crate::commands::NATIVE_COMMAND_QUEUE.lock().unwrap().push(line.to_owned());
                    }
                });
            }
            Err(e) => serror!("RCON Connection failed: {e}"),
        }
    }
}

#[cfg(feature="rcon_commands")]
fn console_say_request(line: &str) -> Option<crate::events::models::CommandRequest> {
    use crate::events::models::{ActorIdentity, ActorPermissions, CommandActor, CommandRequest, CommandSource, PermissionFlags};
    let (name, args, raw_args) = crate::tools::command_parser::parse_command(line)?;
    if !name.eq_ignore_ascii_case("say") { return None; }
    Some(CommandRequest {
        name: "say".into(), args, raw_args,
        actor: CommandActor {
            identity: ActorIdentity::ServerConsole,
            permissions: ActorPermissions { flags: PermissionFlags::all() },
            display_name: "Console".into(),
        },
        source: CommandSource::ServerConsole,
    })
}

#[cfg(all(test, feature="rcon_commands"))]
mod tests {
    use super::*;

    #[test]
    fn rcon_say_uses_the_console_command_path() {
        let command = console_say_request("Say \"hello world\"").unwrap();
        assert_eq!(command.name, "say");
        assert_eq!(command.args, ["hello world"]);
        assert_eq!(command.source, crate::events::models::CommandSource::ServerConsole);
    }

    #[test_case::test_case("teamsay hello"; "team_chat")]
    #[test_case::test_case("!say hello"; "chat_command_prefix")]
    #[test_case::test_case("cmd quit"; "other_command")]
    #[test_case::test_case("say;quit"; "command_separator")]
    #[test_case::test_case("say \"unterminated"; "unclosed_quote")]
    fn rcon_say_rejects_other_commands_and_malformed_input(line: &str) {
        assert!(console_say_request(line).is_none());
    }

    #[test]
    fn rcon_say_keeps_command_looking_text_as_message_arguments() {
        let command = console_say_request("say \"!cmd quit\"").unwrap();
        assert_eq!(command.name, "say");
        assert_eq!(command.args, ["!cmd quit"]);
    }
}
