use std::{
    sync::{Arc, Mutex, Condvar},
    thread,
    time::{Duration, Instant},
};
use a2s::A2SClient;
use once_cell::sync::Lazy;
use reqwest::blocking::Client;

const MAX_RETRIES: u8 = 10;

// Reg start
use serde::{Deserialize, Serialize};

use crate::globals;

#[derive(Serialize)]
struct RegisterRequest<'a> {
    ports: Ports,
    name: &'a str,
    description: &'a str,
    password_protected: bool,
    current_map: &'a str,
    player_count: i32,
    max_players: i32,
    local_ip_address: &'a str,
    mods: &'a [ModInfo<'a>],
}

#[derive(Serialize)]
struct Ports {
    game: u16,
    ping: u16,
    a2s: u16,
}

#[derive(Serialize)]
struct ModInfo<'a> {
    name: &'a str,
    version: &'a str,
}

#[derive(Deserialize)]
struct RegisterResponse {
    server: RegisteredServer,
    key: String,
    refresh_before: f64,
}

#[derive(Deserialize)]
struct RegisteredServer {
    unique_id: String,
}

// Reg end
#[derive(Debug, Serialize)]
struct UpdatePayload<'a> {
    player_count: u8,
    max_players: u8,
    map_name: &'a str,
}
// static mut LAST_INFO:Option<a2s::info::Info> = None;
pub static LAST_INFO: Lazy<Arc<Mutex<Option<a2s::info::Info>>>>   = Lazy::new(|| Arc::new(Mutex::new(None)));

// Info {
//     protocol: 85,
//     name: "Créc",
//     map: "TO_Hippodrome",
//     folder: "TO",
//     game: "Chivalry 2",
//     app_id: 100,
//     players: 1,
//     max_players: 64,
//     bots: 0,
//     server_type: Listen,
//     server_os: Windows,
//     visibility: false,
//     vac: false,
//     the_ship: None,
//     version: "261740",
//     edf: 128,
//     extended_server_info: ExtendedServerInfo {
//         port: Some(
//             7777,
//         ),
//         steam_id: None,
//         keywords: None,
//         game_id: None,
//     },
//     source_tv: None,
// }

// static EMPTY_MODS: &[ModInfo] = &[]; // FIXME: Nihi: yuck
static EMPTY_MODS: &[ModInfo<'static>] = &[];
pub static REGISTRATION: Lazy<Arc<Mutex<Option<Registration>>>>   = Lazy::new(|| Arc::new(Mutex::new(None)));
pub struct Registration {
    server_addr: String,
    query_port: u16,
    client: Client,
    stop_update: Arc<(Mutex<bool>, Condvar)>,
    stop_heartbeat: Arc<(Mutex<bool>, Condvar)>,
    heartbeat_thread: Mutex<Option<thread::JoinHandle<()>>>,
    last_info: Option<a2s::info::Info>
}

fn instant_from_unix_time(unix_secs: f64) -> Option<Instant> {
    // Convert f64 seconds to Duration
    let whole = unix_secs.trunc() as u64;
    let frac = unix_secs.fract();
    let nanos = (frac * 1_000_000_000.0) as u32;

    let sys_time = std::time::UNIX_EPOCH.checked_add(Duration::new(whole, nanos))?;
    let now = std::time::SystemTime::now();

    if sys_time > now {
        // time in the future: compute duration from now
        let dur = sys_time.duration_since(now).ok()?;
        Some(Instant::now() + dur)
    } else {
        // time in the past: compute duration ago
        let dur = now.duration_since(sys_time).ok()?;
        Instant::now().checked_sub(dur)
    }
}

impl Registration {
    pub fn new(ip: &str, query_port: u16) -> Self {
        Self {
            server_addr: format!("{ip}:{query_port}"),
            query_port,
            client: Client::new(),
            stop_update: Arc::new((Mutex::new(false), Condvar::new())),
            stop_heartbeat: Arc::new((Mutex::new(false), Condvar::new())),
            heartbeat_thread: Mutex::new(None),
            last_info: None
        }
    }
    
    pub fn register_server(
        &self,
        server_list_url: &str,
        local_ip: &str,
        game_port: u16,
        ping_port: u16,
        query_port: u16,
        name: &str,
        description: &str,
        current_map: &str,
        player_count: i32,
        max_players: i32,
        // mods: &[ModInfo],
        password_protected: bool,
    // ) -> Result<(String, String, f64), reqwest::Error> {
        
    ) -> Result<(String, String, f64), String> {
        let mods: &[ModInfo] = &[];
        let req_body = RegisterRequest {
            ports: Ports {
                game: game_port,
                ping: ping_port,
                a2s: query_port,
            },
            name,
            description,
            password_protected,
            current_map,
            player_count,
            max_players,
            local_ip_address: local_ip,
            mods,
        };

        let response = self
            .client
            .post(format!("{server_list_url}/api/v1/servers"))
            .json(&req_body)
            .send().unwrap();

        if !response.status().is_success() {
            // You can implement detailed error handling here
            eprintln!("Registration failed: {}", 
            reqwest::StatusCode::from_u16(response.status().as_u16()).unwrap());
            // return Err(reqwest::Error::new(Kind::Request, "Registration Failed"));
            return Err("ERROR".to_string())
            // return Err(reqwest::Error::new(
            //     reqwest::StatusCode::from_u16(response.status().as_u16()).unwrap(),
            //     "Registration failed",
            // ));
        }

        // let parsed: RegisterResponse = response.json()?;
        let resp: Result<(String, String, f64), String> = match response.json() as Result<RegisterResponse, reqwest::Error> {
            Ok(resp) => {
                Ok((
                    resp.server.unique_id,
                    resp.key,
                    resp.refresh_before,
                ))
            },
            _ => {
                eprintln!("NO RESPONSE");
                Err("some".to_string())
            }            
        };
        resp

    }

    pub fn start(&self, server_list_url: &str, id: &str, key: &str) {
        let client = self.client.clone();
        let server_addr = self.server_addr.clone();
        let stop_flag = self.stop_update.clone();
        let id = id.to_string();
        let key = key.to_string();
        let url = server_list_url.to_string();

        thread::spawn(move || {
            let a2s = A2SClient::new().unwrap();
            loop {
                let (lock, cvar) = &*stop_flag;
                if *lock.lock().unwrap() {
                    break;
                }

                let mut retries = 0;
                while retries < MAX_RETRIES {
                    // match a2s.info(&server_addr) {
                    //     Ok(info) => {
                    //         println!("CONNECTED {info:#?}")
                    //     },
                    //     Err(e) => println!("failed {e}"),
                    // }
                    if let Ok(info) = a2s.info(&server_addr) {
                        let payload = UpdatePayload {
                            player_count: info.players,
                            max_players: info.max_players,
                            // map_name: &info.map_name,
                            map_name: info.map.as_str(),
                        };

                        let res = client.post(format!("{url}/update"))
                            .json(&payload)
                            .send();

                        if res.is_ok() {
                            // self.last_info = Some(info.clone());
                            let last_info = Arc::clone(&LAST_INFO);
                            *last_info.lock().unwrap() = Some(info.clone());
                            sdebug!(f; "Updated server: {:?}", payload);
                            break;
                        }
                    }
                    else {
                        sdebug!(f; "Failed to connect to a2s");
                    }

                    retries += 1;
                    sdebug!(f; "Retrying A2S query ({}/{})", retries, MAX_RETRIES);
                    thread::sleep(Duration::from_secs(1));
                }

                let now = Instant::now();
                let timeout = Duration::from_secs(10);
                let _ = cvar.wait_timeout(lock.lock().unwrap(), timeout).unwrap();
            }
        });
    }

    pub fn stop(&self) {
        let (lock, cvar) = &*self.stop_update;
        *lock.lock().unwrap() = true;
        cvar.notify_all();
    }

    pub fn start_heartbeat(self: Arc<Self>, server_list_url: &str, id: &str, key: &str) {
        let stop_flag = self.stop_heartbeat.clone();
        let client = self.client.clone();
        let url = server_list_url.to_string();
        let mut id = id.to_string();
        let mut key = key.to_string();
        let refresh_before: f64 = 0.0;
        let self_clone = Arc::clone(&self);
        let stop_flag = self_clone.stop_heartbeat.clone();

        let handle = thread::spawn(move || {
            let mut refresh_by = Instant::now() + Duration::from_secs(30); // default interval

            loop {
                let (lock, cvar) = &*stop_flag;
                if *lock.lock().unwrap() {
                    break;
                }

                // Only send heartbeat if it's near refresh_by time
                if Instant::now() >= refresh_by {
                    // Send heartbeat POST/GET, here assumed POST to "/heartbeat"
                    // Replace with your actual heartbeat logic & parsing

                    let res = client.post(format!("{url}/heartbeat"))
                        .json(&serde_json::json!({
                            "id": id,
                            "key": key,
                        }))
                        .send();

                    match res {
                        Ok(resp) if resp.status().is_success() => {
                            // If your server returns next refresh interval, parse it here.
                            // For demo, assume fixed 30s interval:
                            refresh_by = Instant::now() + Duration::from_secs(30);
                            sdebug!(f; "Heartbeat successful");
                        }
                        Ok(resp) if resp.status().as_u16() == 404 => {
                            sdebug!(f; "Registration expired; re-register here");
                            
                            let last_info = Arc::clone(&LAST_INFO);
                            // *last_info.lock().unwrap() = Some(info.clone());
                            let empty: &[ModInfo<'_>] = &[];
                            if let Some(info) = LAST_INFO.lock().unwrap().as_ref() {
                                let args = &globals().cli_args;   
                                let name = format!("{}\n(local server)", info.name);
                                let txt = format!("{} (build {})\n{} server ", info.game, info.version, info.folder);
                                
                                let res = self_clone.register_server(
                                    &url,
                                    "127.0.0.1",
                                    info.extended_server_info.port.unwrap(),
                                    args.game_server_ping_port.unwrap(),
                                    args.game_server_query_port.unwrap(),
                                    &name,
                                    &txt,
                                    &info.map,
                                    info.players as i32,
                                    info.max_players as i32,
                                    // EMPTY_MODS,
                                    args.server_password.is_some(),
                                    );

                                // sdebug!(f; "INFO: {info:#?}");

                                match res {
                                    Ok((unique_id, new_key, refresh)) => { 
                                        id = unique_id;                                       
                                        key = new_key;
                                        // let test = Instant::now() + (Duration::from_secs_f64(refresh) - std::time::UNIX_EPOCH);
                                        refresh_by = instant_from_unix_time(refresh).unwrap() - Duration::from_secs(5);
                                    },
                                    Err(e) => serror!(f; "FAILED TO REGISTER {e}"),
                                }
                            }
                            else {
                                serror!(f; "No info saved");
                                refresh_by = Instant::now() + Duration::from_secs(5);
                            }

                            // TODO: re-register logic (POST register, update id/key)
                        }
                        Ok(resp) => {
                            serror!(f; "Heartbeat failed with status: {}", resp.status());
                        }
                        Err(e) => {
                            serror!(f; "Heartbeat error: {}", e);
                        }
                    }
                }

                // Sleep until next check or stop signal
                let wait_time = refresh_by.saturating_duration_since(Instant::now());
                let lock_guard = lock.lock().unwrap();
                if *cvar.wait_timeout(lock_guard, wait_time).unwrap().0 {
                    break;
                }
            }
        });

        *self.heartbeat_thread.lock().unwrap() = Some(handle);
    }

    pub fn stop_heartbeat(&self) {
        let (lock, cvar) = &*self.stop_heartbeat;
        *lock.lock().unwrap() = true;
        cvar.notify_all();

        if let Some(handle) = self.heartbeat_thread.lock().unwrap().take() {
            handle.join().unwrap();
        }
    }
}
