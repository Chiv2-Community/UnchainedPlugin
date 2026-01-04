use clap::{CommandFactory, Parser};

#[derive(Parser, Debug)]
#[command(name = "Chivalry 2 Unchained", author = "Unchained Team", version, about, long_about = None)]
// FIXME: possible to skip unhandled args?
pub struct CLIArgs {
    // #[arg()]
    // positional_args: Vec<String>,
    // #[arg()]
    // game_id: String,
    #[arg(long = "next-map-mod-actors", value_delimiter = ',', required = false)]
    pub next_mod_actors: Option<Vec<String>>,

    #[arg(long = "all-mod-actors", value_delimiter = ',', required = false)]
    pub mod_paks: Option<Vec<String>>,

    #[arg(long = "unchained")]
    pub is_unchained: bool,
    //
    #[arg(long = "rcon")]
    pub rcon_port: Option<u16>,
    // //
    #[arg(long = "desync-patch")]
    pub apply_desync_patch: bool,
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
    #[arg(trailing_var_arg = true)]
    pub extra_args: Vec<String>,
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
    let mut args = args.peekable();
    let mut last_flag: Option<String> = None;
    let mut last_opt: Option<String> = None;

    while let Some(arg) = args.next() {
        // println!("-- LINE: {arg}");
        // Normalize `key=value` and `-flag` → `--flag`
        let (flag, value_opt): (String, Option<String>) = if let Some((k, v)) = arg.split_once('=')
        {
            // println!("{} Split by =", arg);
            // println!("k '{}' , v '{}' ", k, v);
            (
                format!("--{}", k.trim_start_matches('-')),
                Some(v.to_string()),
            )
        } else if arg.starts_with('-') && !arg.starts_with("--") && arg.len() > 2 {
            // println!("{} Split by -", arg);
            (format!("--{}", &arg[1..]), None)
        } else {
            // println!("{} Split by else", arg);
            (arg.clone(), None)
        };

        // println!("cur: '{flag}'");
        let cur_flag = flag.clone();
        if known_flags.contains(&flag) {
            result.push(flag.trim().to_string());
            if let Some(v) = value_opt {
                // println!("option: '{v}'");
                last_opt = Some(v.clone());
                result.push(v.trim().to_string());
            } else if let Some(peek) = args.peek() {
                if !peek.starts_with('-') {
                    let var = args.next().unwrap();
                    // print!("pushing '{var}'");
                    result.push(var.trim().to_string());
                }
            }
        }
        // args can split an option (e.g. --name Not Sure)
        else if !result.is_empty() && !flag.starts_with('-') {
            let last_valid = result.last().unwrap();
            if last_flag.is_some() {
                // println!("Last '{last}' last valid '{last_valid}'");
                if let Some(o) = &last_opt {
                    // println!("Last '{}' last valid {} last option '{}' equal: {}", last, last_valid, o, o == last_valid);
                    // println!("Res: {} Trailing string {}, last flag {}, last result {}",result.len(), flag, last, last_valid);
                    if o == last_valid {
                        if let Some(last_mut) = result.last_mut() {
                            if !cur_flag.is_empty() {
                                // println!("Trailing '{cur_flag}'");
                                last_mut.push(' ');
                                last_mut.push_str(cur_flag.trim());
                            }
                            // last_mut = last_mut.trim().to_string();
                        }
                    }
                }
            }
        }
        last_flag = Some(cur_flag);
    }

    // println!("Res: {:?}", result);
    result
}

pub unsafe fn load_cli() -> Result<CLIArgs, clap::error::Error> {
    let args = std::env::args();
    let parsed = normalize_and_filter_args(args);
    let cli = CLIArgs::try_parse_from(parsed).expect("Failed to parse CLI args");
    Ok(cli)
}