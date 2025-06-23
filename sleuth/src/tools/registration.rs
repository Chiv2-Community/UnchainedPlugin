use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use a2s::A2SClient;
// use anyhow::Ok;
// use anyhow::Ok;
// use once_cell::sync::Lazy;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::globals;

// Register Request
#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
struct ModInfo<'a> {
    name: &'a str,
    version: &'a str,
}

#[derive(Debug, Serialize)]
struct Ports {
    game: u16,
    ping: u16,
    a2s: u16,
}

// TODO: Maybe use Identity instead
#[derive(Debug, Deserialize, Clone)]
struct RegisterResponse {
    server: RegisteredServer,
    key: String,
    refresh_before: f64,
}

impl RegisterResponse {
    fn refresh_eta_secs(&self) -> u64 {        
        let start = SystemTime::now();
        let since_epoch = start.duration_since(UNIX_EPOCH).expect("Time went backwards");
        sinfo!(f; "ETA: {}", self.refresh_before as u64 - since_epoch.as_secs());
        self.refresh_before as u64 - since_epoch.as_secs()
    }
}

#[derive(Debug, Deserialize, Clone)]
struct RegisteredServer {
    unique_id: String,
}

// Server id/key
struct Identity {
    id: String,
    key: String,
    refresh_period: u64,
}

impl From<RegisterResponse> for Identity {
    fn from(res: RegisterResponse) -> Self {
        Identity {
            id: res.server.unique_id.clone(),
            key: res.key.clone(),
            refresh_period: res.refresh_eta_secs()
        }
    }
}

impl Identity {
    fn default() -> Self {
        Self {
            id: String::new(),
            key: String::new(),
            refresh_period: 60,
        }
    }

    pub fn new(id: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            key: key.into(),
            refresh_period: 60,
        }
    }
}

#[derive(Debug, Serialize)]
struct UpdatePayload<'a> {
    player_count: u8,
    max_players: u8,
    map_name: &'a str,
}

pub struct Registration{
    server_addr: String,
    client: Client,
    a2s: A2SClient,
    identity: Mutex<Option<Identity>>,
    stop_update: Arc<(Mutex<bool>, Condvar)>,
    stop_heartbeat: Arc<(Mutex<bool>, Condvar)>,
    heartbeat_thread: Mutex<Option<thread::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>>>,
}
impl Registration {
    pub fn new(ip: &str, query_port: u16) -> Self {
        Self {
            server_addr: format!("{ip}:{query_port}"),
            // query_port,
            client: Client::new(),
            a2s: A2SClient::new().unwrap(),
            identity: Mutex::new(None),
            stop_update: Arc::new((Mutex::new(false), Condvar::new())),
            stop_heartbeat: Arc::new((Mutex::new(false), Condvar::new())),
            heartbeat_thread: Mutex::new(None),
            // last_info: None
        }
    }

    fn register_server(self: Arc<Self>, info: a2s::info::Info) -> Result<RegisterResponse, reqwest::Error> {
        let args = &globals().cli_args;   
        let desc = format!("{} (build {})\n{} server ",
         info.game,
         info.version,
         info.folder);
        let request = RegisterRequest{
            ports: Ports {
                game: info.extended_server_info.port.unwrap(),
                ping: args.game_server_ping_port.unwrap(),
                a2s: args.game_server_query_port.unwrap(),
            },
            name: &info.name,
            description: &desc,
            password_protected: args.server_password.is_some(), // TODO
            current_map: &info.map,
            player_count: info.players as i32,
            max_players: info.max_players as i32,
            local_ip_address: "127.0.0.1",
            mods: &[],
        };
        let backend = args.server_browser_backend.clone();
        let response = self.client
            .post(format!("{}/api/v1/servers", &backend.unwrap()))
            .json(&request)
            .send().unwrap();

        swarn!(f; "request {:#?}", request);
        swarn!(f; "response {:#?}", response);
        match response.json() as Result<RegisterResponse, reqwest::Error> {
            Ok(res) => {
                swarn!(f; "json {:#?}", res);
                
                let mut identity_lock = self.identity.lock().unwrap();
                // *identity_lock = Some(Identity::new(&res.server.unique_id, &res.key));
                *identity_lock = Some(res.clone().into());
                Ok(res)
            },
            Err(e) => { serror!(f; "Error: {}", e); Err(e) },            
        }
    }

    fn a2s_get_info(&self, retries: usize, period_s: f32) -> Result<a2s::info::Info, Box<dyn std::error::Error>> {
        let stop_flag = self.stop_update.clone();
        let mut itr = 0;
        while itr < retries {
            let (lock, cvar) = &*stop_flag;
            if *lock.lock().unwrap() {
                break;
            }
            sinfo!(f; "Retry {itr}");
            match self.a2s.info(&self.server_addr) {
                Result::Ok(info) =>  {
                    serror!(f; "Connected {:#?}", info);
                    // serror!(f; "Players {:#?}", self.a2s.players(&self.server_addr)); // !
                    // serror!(f; "Rules {:#?}", self.a2s.rules(&self.server_addr));
                    return Ok(info)
                },
                Err(e) => serror!(f; "Failed to get info: {e}"),
            }
            itr += 1;
            if itr < retries { // TODO: do..while?
                thread::sleep(Duration::from_secs(1));
            }
        }
        Err(format!("A2S failed after {retries} retries.").into())
    }

    fn start_discovery(self: Arc<Self>) -> Result<RegisterResponse, Box<dyn std::error::Error + Send + Sync>> {
        
        match self.a2s_get_info(50, 1.0) {
            Ok(info) => {
                sinfo!(f; "discovered server!");
                match self.register_server(info) {
                    Ok(rinfo) => {
                        sinfo!(f; "ok");
                        Ok(rinfo)
                    }, // TODO: handle player list?
                    Err(e) => {
                        serror!(f; "Not ok :{e}"); 
                        Err(e.into())
                    },
                }
            },
            Err(e) => {
                serror!(f; "Error: {}", e);
                Err(format!("Error: {}", e).into())
            },
        }
        // Err("Reached max retries".into())
    }

    pub fn start_a2s(self: Arc<Self>) {
        let reg = self.clone();

        thread::spawn(move || {
            // reg.start_a2s();
        });
    }

    pub fn start(self: Arc<Self>) {
        // self.clone().start_heartbeat();
        // self.clone().start_a2s();
        let reg = self.clone();
    
        // spawn discovery thread
        thread::spawn(move || {
            thread::sleep(Duration::from_secs(20));
            match reg.start_discovery() {
                Ok(_) => {
                    println!("Discovery successful, starting heartbeat...");
                    match self.start_heartbeat() {
                        Ok(_) => sinfo!(f; "Heartbeat started"),
                        Err(e) => serror!(f; "Error: {e}"),
                    }
                    // Register server
                    // start heartbeat
                    // reg.start_heartbeat();
                }
                Err(e) => {
                    eprintln!("Discovery failed: {:?}", e);
                }
            }
        });
    }

    pub fn stop(self: Arc<Self>) {

    }

    fn send_update(self: Arc<Self>) -> Result<(), Box<dyn std::error::Error + Send + Sync>>{
        sinfo!(f; "start");
        let client = self.client.clone();
        // let url = self.server_addr.clone();
        let args = &globals().cli_args;   
        let backend = args.server_browser_backend.clone().unwrap();
        sinfo!(f; "getting lock");
        let identity_lock: std::sync::MutexGuard<'_, Option<Identity>> = self.identity.lock().unwrap();
        sinfo!(f; "getting ident");
        let ident = identity_lock.as_ref()
            .ok_or("Heartbeat with unknown identity")?;
        sinfo!(f; "getting info");
        match self.a2s_get_info(1, 0.0) {
            Ok(info) => {
                sinfo!(f; "got a2s info");
                let payload = UpdatePayload {
                    player_count: info.players,
                    max_players: info.max_players,
                    map_name: info.map.as_str(),
                };
                
                let res = client.post(format!("{}/update", backend))
                    .json(&payload)
                    .send();

                match res {
                    Ok(update) => {
                        sinfo!(f; "sent update payload {:#?}", update);
                        // sinfo!("Sending update!");
                        let res_hb = client
                            .post(format!("{}/{}/heartbeat", backend, ident.id))
                            .header("x-chiv2-server-browser-key", ident.key.as_str())
                            .send();

                        match res_hb {
                            Ok(hb) => {
                                sinfo!("Sent heartbeat!");
                                Ok(())
                            },
                            Err(e) =>  Err(format!("Update: failed to send heartbeat: {}", e).into()),
                        }
                    },
                    Err(e) =>  Err(format!("Update: failed to send update: {}", e).into()),
                }

                
        
                // FIXME: why not derived here?
                // Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            }

            Err(e) => Err(format!("Update: failed to get A2S: {}", e).into()),
        }
    }

    pub fn start_heartbeat(self: Arc<Self>) -> Result<(), Box<dyn std::error::Error>> {
        sinfo!(f; "start");
        let stop_signal = Arc::clone(&self.stop_heartbeat);
        let this = Arc::clone(&self);
        // let identity = self.identity.lock().unwrap();
        // match self.identity.lock().unwrap().as_ref() {
        //     Some(ident) => todo!(),
        //     None => todo!(),
        // };
        let handle = thread::spawn(move || -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let (lock, cvar) = &*stop_signal;
            loop {
                let mut stopped = lock.lock().unwrap();
                
                if *stopped {
                    println!("🛑 Heartbeat stopping.");
                    break;
                }


                match this.clone().send_update() {
                    Ok(_) => sinfo!(f; "upadte success"),
                    Err(e) => sinfo!(f; "upadte error: {}", e),
                };

                let identity_lock = this.identity.lock().unwrap();
                let ident = identity_lock.as_ref()
                    .ok_or("Heartbeat with unknown identity")?;

                let result = cvar.wait_timeout(stopped, Duration::from_secs(ident.refresh_period)).unwrap();
                stopped = result.0;
                // if *stopped {
                //     println!("🛑 Heartbeat stopping.");
                //     break;
                // }
            }
            Ok(())
        });

        *self.heartbeat_thread.lock().unwrap() = Some(handle);
        Ok(())
    }

    pub fn stop_heartbeat(self: Arc<Self>) {

    }
    
}