use std::collections::HashMap;

use clap::{CommandFactory, Parser};

use crate::sdebug;

pub type IniMap = HashMap<String, HashMap<String, HashMap<String, String>>>;

#[derive(Parser, Debug)]
#[command(name = "Chivalry 2 Unchained", author = "Unchained Team", version, about, long_about = None)]
// FIXME: possible to skip unhandled args?
pub struct CLIArgs {
    // #[arg()]
    // positional_args: Vec<String>,
    // #[arg()]
    // game_id: String,
    #[deprecated]
    #[arg(long = "next-map-mod-actors", value_delimiter = ',', required = false)]
    pub next_mod_actors: Option<Vec<String>>,
    #[deprecated]
    #[arg(long = "all-mod-actors", value_delimiter = ',', required = false)]
    pub mod_paks: Option<Vec<String>>,

    #[arg(long = "server-mods", value_delimiter = ',', required = false)]
    pub server_mods: Option<Vec<String>>,

    #[arg(skip)]
    pub ini_overrides: IniMap,

    #[arg(long = "unchained")]
    pub is_unchained: bool,
    //
    #[arg(long = "rcon")]
    pub rcon_port: Option<u16>,
    // //
    #[arg(long = "desync-patch")]
    pub apply_desync_patch: bool,
    // //
    
    #[cfg(feature="move-autonomous-desync")]
    #[arg(long = "tick-actor-patch")]
    pub tick_actor_patch: bool,
    //
    #[arg(long = "use-backend-banlist")]
    pub use_backend_banlist: bool,
    //
    #[arg(long = "nullrhi")]
    pub is_headless: bool,
    //
    #[arg(long = "next-map-name")]
    pub next_map: Option<String>,
    //
    #[arg(long = "launched-profile")]
    pub launched_profile: Option<String>,
    //
    #[arg(long = "playable-listen")]
    pub playable_listen: bool,
    //
    #[arg(long = "register")]
    pub register: bool,
    // //
    #[arg(
        long = "server-browser-backend",
        default_value = "https://servers.polehammer.net"
    )]
    pub server_browser_backend: Option<String>,
    // //
    #[arg(long = "server-password")]
    pub server_password: Option<String>,
    // s
    #[arg(long = "platform")]
    pub platform: Option<String>,

    #[arg(long = "GameServerPingPort", default_value = "3075")]
    pub game_server_ping_port: Option<u16>,

    #[arg(long = "GameServerQueryPort", default_value = "7071")]
    pub game_server_query_port: Option<u16>,

    #[arg(long = "Port", default_value = "7777")]
    pub game_port: Option<u16>,

    // #[cfg(feature="discord_integration_old")]
    #[arg(long = "discord-channel-id")]
    pub discord_channel_id: Option<u64>,
    
    #[arg(long = "discord-admin-channel-id")]
    pub discord_admin_channel_id: Option<u64>,

    #[arg(long = "discord-general-channel-id")]
    pub discord_general_channel_id: Option<u64>,

    #[arg(long = "discord-admin-role-id")]
    pub discord_admin_role_id: Option<u64>,

    // #[cfg(feature="discord_integration_old")]
    #[arg(long = "discord-bot-token")]
    pub discord_bot_token: Option<String>,

    // UNHANDLED START
    // #[arg(long = "AUTH_LOGIN")]
    // auth_login: Option<String>,
    // #[arg(long = "AUTH_PASSWORD")]
    // auth_password: Option<String>,
    // #[arg(long = "AUTH_TYPE")]
    // auth_type: Option<String>,
    // #[arg(long = "epicapp")]
    // epicapp: Option<String>,
    // #[arg(long = "epicenv")]
    // epicenv: Option<String>,
    // #[arg(long = "EpicPortal")]
    // epic_portal: bool,
    // #[arg(long = "epicusername")]
    // epicusername: Option<String>,
    // #[arg(long = "epicuserid")]
    // epicuserid: Option<String>,
    // #[arg(long = "epiclocale")]
    // epiclocale: Option<String>,
    // #[arg(long = "epicsandboxid")]
    // epicsandboxid: Option<String>,
    // UNHANDLED END
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub extra_args: Vec<String>,
}

impl CLIArgs {
    fn process_ini_map(raw_args: &[String]) -> IniMap {
        let mut map: IniMap = HashMap::new();
        for arg in raw_args {
            if let Some(stripped) = arg.strip_prefix("-ini:") {
                // Split into File, Section, and Key=Value
                let parts: Vec<&str> = stripped.splitn(3, ':').collect();
                if let [file, section, kv_pair] = parts.as_slice() {
                    if let Some((key, value)) = kv_pair.split_once('=') {
                        map.entry(file.to_string())
                            .or_default()
                            .entry(section.to_string())
                            .or_default()
                            .insert(key.to_string(), value.to_string());
                    }
                }
            }
        }
        map
    }

    // #[cfg(feature="discord_integration_old")]
    pub fn discord_enabled(&self) -> bool {
        self.is_server() && self.discord_bot_token.is_some() && self.discord_channel_id.is_some()
    }

    pub fn is_server(&self) -> bool {
        self.rcon_port.is_some()
    }

    pub fn find_ini_value(&self, paths: &[(&str, &str, &str)]) -> Option<&str> {
        paths.iter().find_map(|(file, section, key)| {
            self.ini_overrides
                .get(*file)?
                .get(*section)?
                .get(*key)
                .map(|s| s.as_str()) // Explicitly convert to &str before the closure returns
        })
    }
}

// We're using a mix of cli arg types, normalize them to --key value(s)
// e.g. -rcon 9001, -epicsomething=blablabla, Port=7777
// This function converts all of those to --convention. It also drops game_identifier
// To parse it all with clap, it checks against entries in CLIArgs and filters out unhandled ones
fn normalize_and_filter_args<I: IntoIterator<Item = String>>(args: I) -> Vec<String> {
    let mut args = args.into_iter();
    let bin_name = args.next().unwrap_or_else(|| "app".to_string());

    let known_flags: Vec<String> = CLIArgs::command()
        .get_arguments()
        .filter_map(|a| a.get_long().map(|s| format!("--{s}")))
        .collect();

    let mut result = vec![bin_name];
    let mut ini_args = vec![]; 
    let mut args = args.peekable();

    while let Some(arg) = args.next() {
        // 1. Short-circuit INIs so they don't break flag/value pairs
        if arg.starts_with("-ini:") {
            ini_args.push(arg);
            continue;
        }

        // 2. Split or Normalize
        let (flag, value_opt): (String, Option<String>) = if let Some((k, v)) = arg.split_once('=') {
            (format!("--{}", k.trim_start_matches('-')), Some(v.to_string()))
        } else if arg.starts_with('-') && !arg.starts_with("--") && arg.len() > 2 {
            (format!("--{}", &arg[1..]), None)
        } else {
            (arg.clone(), None)
        };

        // 3. Match against known flags
        if known_flags.contains(&flag) {
            result.push(flag);
            if let Some(v) = value_opt {
                result.push(v);
            } else {
                // Peek for the value if not provided via '='
                while let Some(peek) = args.peek() {
                    if !peek.starts_with('-') && !peek.contains('=') {
                        result.push(args.next().unwrap());
                    } else {
                        break;
                    }
                }
            }
        }
    }

    // need to add -ini fields at the end, otherwise clap chokes
    result.extend(ini_args);
    result
}

pub fn load_cli() -> Result<CLIArgs, clap::error::Error> {
    let args = std::env::args();
    sdebug!(f; "CLI Args raw: {:#?}", args);
    let parsed = normalize_and_filter_args(args);
    let mut cli = CLIArgs::try_parse_from(parsed)?;
    cli.ini_overrides = CLIArgs::process_ini_map(&cli.extra_args);
    Ok(cli)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_invalid_int() {
        let args = vec!["app".to_string(), "--rcon".to_string(), "not_an_int".to_string()];
        let result = CLIArgs::try_parse_from(args);
        assert!(result.is_err(), "Should fail to parse invalid integer");
    }

    #[test]
    fn test_parse_valid_args() {
        let args = vec!["app".to_string(), "--rcon".to_string(), "9001".to_string()];
        let result = CLIArgs::try_parse_from(args);
        assert!(result.is_ok(), "Should parse valid arguments");
        assert_eq!(result.unwrap().rcon_port, Some(9001));
    }

    #[test]
    fn test_normalize_ue_style_args() {
        let args = vec![
            "app".to_string(),
            "-rcon".to_string(), "9001".to_string(),
            "Port=7777".to_string(),
            "-nullrhi".to_string(),
            "-ini:Game:/Script/Engine.GameSession:MaxPlayers=64".to_string(),
            "UnknownArg=Value".to_string(),
            "-UnknownFlag".to_string(),
        ];
        let normalized = normalize_and_filter_args(args);
        
        // Should include: app, --rcon, 9001, --Port, 7777, --nullrhi, -ini:...
        // Should NOT include: UnknownArg=Value, -UnknownFlag
        assert_eq!(normalized[0], "app");
        assert!(normalized.contains(&"--rcon".to_string()));
        assert!(normalized.contains(&"9001".to_string()));
        assert!(normalized.contains(&"--Port".to_string()));
        assert!(normalized.contains(&"7777".to_string()));
        assert!(normalized.contains(&"--nullrhi".to_string()));
        assert!(normalized.contains(&"-ini:Game:/Script/Engine.GameSession:MaxPlayers=64".to_string()));
        
        // Ensure unknown args are filtered out
        assert!(!normalized.contains(&"UnknownArg=Value".to_string()));
        assert!(!normalized.contains(&"--UnknownArg".to_string()));
        assert!(!normalized.contains(&"--UnknownFlag".to_string()));
        
        // Check order of -ini (should be at the end)
        assert_eq!(normalized.last().unwrap(), "-ini:Game:/Script/Engine.GameSession:MaxPlayers=64");
    }

    #[test]
    fn test_process_ini_map() {
        let extra_args = vec![
            "-ini:Game:/Script/Engine.GameSession:MaxPlayers=64".to_string(),
            "-ini:Engine:[Core.Log]:LogHttp=VeryVerbose".to_string(),
            "not-an-ini".to_string(),
        ];
        let ini_map = CLIArgs::process_ini_map(&extra_args);

        assert_eq!(
            ini_map.get("Game").unwrap().get("/Script/Engine.GameSession").unwrap().get("MaxPlayers").unwrap(),
            "64"
        );
        assert_eq!(
            ini_map.get("Engine").unwrap().get("[Core.Log]").unwrap().get("LogHttp").unwrap(),
            "VeryVerbose"
        );
    }

    #[test]
    fn test_find_ini_value() {
        let mut ini_overrides = HashMap::new();
        let mut engine_map = HashMap::new();
        let mut log_map = HashMap::new();
        log_map.insert("LogHttp".to_string(), "VeryVerbose".to_string());
        engine_map.insert("[Core.Log]".to_string(), log_map);
        ini_overrides.insert("Engine".to_string(), engine_map);

        let cli = CLIArgs {
            next_mod_actors: None,
            mod_paks: None,
            server_mods: None,
            ini_overrides,
            is_unchained: false,
            rcon_port: None,
            apply_desync_patch: false,
            #[cfg(feature="move-autonomous-desync")]
            tick_actor_patch: false,
            use_backend_banlist: false,
            is_headless: false,
            next_map: None,
            launched_profile: None,
            playable_listen: false,
            register: false,
            server_browser_backend: None,
            server_password: None,
            platform: None,
            game_server_ping_port: None,
            game_server_query_port: None,
            game_port: None,
            discord_channel_id: None,
            discord_admin_channel_id: None,
            discord_general_channel_id: None,
            discord_admin_role_id: None,
            discord_bot_token: None,
            extra_args: vec![],
        };

        let paths = [
            ("Engine", "[Core.Log]", "LogHttp"),
            ("Game", "/Script/Engine.GameSession", "MaxPlayers"),
        ];

        assert_eq!(cli.find_ini_value(&paths), Some("VeryVerbose"));
        
        let paths_missing = [
            ("Missing", "Section", "Key"),
        ];
        assert_eq!(cli.find_ini_value(&paths_missing), None);
    }

    #[test]
    fn test_is_server() {
        let mut cli = CLIArgs {
            next_mod_actors: None,
            mod_paks: None,
            server_mods: None,
            ini_overrides: HashMap::new(),
            is_unchained: false,
            rcon_port: None,
            apply_desync_patch: false,
            #[cfg(feature="move-autonomous-desync")]
            tick_actor_patch: false,
            use_backend_banlist: false,
            is_headless: false,
            next_map: None,
            launched_profile: None,
            playable_listen: false,
            register: false,
            server_browser_backend: None,
            server_password: None,
            platform: None,
            game_server_ping_port: None,
            game_server_query_port: None,
            game_port: None,
            discord_channel_id: None,
            discord_admin_channel_id: None,
            discord_general_channel_id: None,
            discord_admin_role_id: None,
            discord_bot_token: None,
            extra_args: vec![],
        };

        assert!(!cli.is_server());
        cli.rcon_port = Some(9001);
        assert!(cli.is_server());
    }

    #[test]
    fn test_process_ini_map_complex() {
        let extra_args = vec![
            "-ini:Game:/Script/Engine.GameSession:MaxPlayers=64".to_string(),
            "-ini:Game:/Script/Engine.GameSession:ServerName=MyServer".to_string(),
            "-ini:Engine:[Core.Log]:LogHttp=VeryVerbose".to_string(),
        ];
        let ini_map = CLIArgs::process_ini_map(&extra_args);

        let game_session = ini_map.get("Game").unwrap().get("/Script/Engine.GameSession").unwrap();
        assert_eq!(game_session.get("MaxPlayers").unwrap(), "64");
        assert_eq!(game_session.get("ServerName").unwrap(), "MyServer");
        
        assert_eq!(
            ini_map.get("Engine").unwrap().get("[Core.Log]").unwrap().get("LogHttp").unwrap(),
            "VeryVerbose"
        );
    }

    #[test]
    fn test_normalize_mixed_order() {
        let args = vec![
            "app".to_string(),
            "-ini:G:S:K=V".to_string(),
            "--rcon".to_string(), "9001".to_string(),
            "-unchained".to_string(),
        ];
        let normalized = normalize_and_filter_args(args);
        
        // -ini should be at the end
        assert_eq!(normalized[0], "app");
        assert_eq!(normalized[1], "--rcon");
        assert_eq!(normalized[2], "9001");
        assert_eq!(normalized[3], "--unchained");
        assert_eq!(normalized[4], "-ini:G:S:K=V");
    }

    #[test]
    fn test_extra_args_handling() {
        let args = vec![
            "app".to_string(),
            "--rcon".to_string(), "9001".to_string(),
            "SomethingElse".to_string(),
            "-UnknownFlag".to_string(),
        ];
        let normalized = normalize_and_filter_args(args);

        assert_eq!(normalized, vec!["app".to_string(), "--rcon".to_string(), "9001".to_string(), "SomethingElse".to_string()]);
        assert!(!normalized.contains(&"-UnknownFlag".to_string()));
        assert!(!normalized.contains(&"--UnknownFlag".to_string()));
    }
}
