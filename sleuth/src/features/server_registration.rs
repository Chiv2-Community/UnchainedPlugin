use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use a2s::A2SClient;

use crate::{globals, serror, sinfo, swarn};

const INITIAL_DELAY_SECS: u64 = 20;
const MAX_HEARTBEAT_SECS: u64 = 60; // Cap heartbeat interval
const DEBUG_LOGGING: bool = true;

#[derive(Debug, Serialize)]
struct RegisterRequest<'a> {
    ports: Ports,
    name: &'a str,
    description: String, // Restored dynamic description
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

#[derive(Debug, Deserialize, Clone)]
struct RegisterResponse {
    server: RegisteredServer,
    key: String,
    refresh_before: f64,
}

#[derive(Debug, Deserialize, Clone)]
struct RegisteredServer {
    unique_id: String,
}

#[derive(Clone)]
struct Identity {
    id: String,
    key: String,
    refresh_at: u64, // Absolute unix timestamp
    refresh_period: u64,
}

impl From<RegisterResponse> for Identity {
    fn from(res: RegisterResponse) -> Self {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let expiry = res.refresh_before as u64;
        
        // Refresh 3x faster than requested, but cap it at our MAX_HEARTBEAT_SECS
        let calculated_period = if expiry > now {
            ((expiry - now) / 3).clamp(10, MAX_HEARTBEAT_SECS)
        } else {
            10
        };

        Identity {
            id: res.server.unique_id,
            key: res.key,
            refresh_at: expiry,
            refresh_period: calculated_period,
        }
    }
}

#[derive(Debug, Serialize)]
struct UpdatePayload<'a> {
    player_count: i32,
    max_players: i32,
    #[serde(rename = "current_map")]
    map_name: &'a str,
}

#[derive(PartialEq)]
enum WorkerState {
    Idle,
    Running,
    StopRequested,
}

pub struct Registration {
    inner: Arc<RegistrationInner>,
    handle: Mutex<Option<thread::JoinHandle<()>>>,
}

struct RegistrationInner {
    server_addr: String,
    state: Mutex<WorkerState>,
    identity: Mutex<Option<Identity>>,
    cvar: Condvar,
    client: Client,
}

impl Registration {
    pub fn new(ip: &str, query_port: u16) -> Self {
        Self {
            inner: Arc::new(RegistrationInner {
                server_addr: format!("{ip}:{query_port}"),
                state: Mutex::new(WorkerState::Idle),
                identity: Mutex::new(None),
                cvar: Condvar::new(),
                client: Client::new(),
            }),
            handle: Mutex::new(None),
        }
    }

    pub fn start(self: Arc<Self>) {
        let mut state_lock = self.inner.state.lock().unwrap();
        if *state_lock == WorkerState::Running { return; }
        *state_lock = WorkerState::Running;

        let inner = Arc::clone(&self.inner);
        let mut handle_lock = self.handle.lock().unwrap();
        
        *handle_lock = Some(thread::spawn(move || {
            // Check if we already have a valid identity (Resume scenario)
            let has_valid_id = {
                let id_lock = inner.identity.lock().unwrap();
                if let Some(id) = id_lock.as_ref() {
                    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                    id.refresh_at > now
                } else {
                    false
                }
            };

            // Only delay if we are starting fresh
            if !has_valid_id {
                let (lock, cvar) = (&inner.state, &inner.cvar);
                let mut state = lock.lock().unwrap();
                let _ = cvar.wait_timeout(state, Duration::from_secs(INITIAL_DELAY_SECS)).unwrap();
            }
            
            inner.run_loop();
        }));
    }

    /// Triggers an immediate A2S poll and API update (e.g., on player join).
    pub fn trigger_update(&self) {
        self.inner.cvar.notify_all();
    }

    /// Stops the heartbeat thread. 
    /// If `deregister` is true, it removes the server from the backend list.
    /// If `false`, it just pauses updates (allowing resume with same key later).
    pub fn stop(&self, deregister: bool) {
        {
            let mut state = self.inner.state.lock().unwrap();
            *state = WorkerState::StopRequested;
        }
        self.inner.cvar.notify_all();

        if let Some(handle) = self.handle.lock().unwrap().take() {
            let _ = handle.join();
        }

        if deregister {
            self.perform_deregistration();
            let mut id_lock = self.inner.identity.lock().unwrap();
            *id_lock = None;
        }
        
        let mut state = self.inner.state.lock().unwrap();
        *state = WorkerState::Idle;
    }

    fn perform_deregistration(&self) {
        let ident_opt = self.inner.identity.lock().unwrap().clone();
        if let Some(ident) = ident_opt {
            let backend = globals().cli_args.server_browser_backend.clone().unwrap_or_default();
            let _ = self.inner.client.delete(format!("{}/api/v1/servers/{}", backend, ident.id))
                .header("X-CHIV2-SERVER-BROWSER-KEY", &ident.key)
                .send();
            if DEBUG_LOGGING { sinfo!(f; "Server deregistered: {}", ident.id); }
        }
    }
}

impl RegistrationInner {
    fn run_loop(&self) {
        let a2s = A2SClient::new().unwrap();
        
        loop {
            if self.is_stopping() { break; }

            // 1. Validate/Get Identity
            let ident = self.get_or_register(&a2s);
            if ident.is_none() {
                thread::sleep(Duration::from_secs(10));
                continue;
            }
            let ident = ident.unwrap();

            // 2. Heartbeat & Update
            if let Err(is_fatal) = self.perform_update_and_heartbeat(&a2s, &ident) {
                if is_fatal { 
                    let mut lock = self.identity.lock().unwrap();
                    *lock = None;
                    break; 
                }
            }

            // 3. Sleep until next cycle or notification
            let state_lock = self.state.lock().unwrap();
            let (new_lock, _) = self.cvar.wait_timeout(state_lock, Duration::from_secs(ident.refresh_period)).unwrap();
            if *new_lock == WorkerState::StopRequested { break; }
        }
    }

    fn get_or_register(&self, a2s: &A2SClient) -> Option<Identity> {
        let mut lock = self.identity.lock().unwrap();
        
        // Check if existing identity is still valid
        if let Some(id) = lock.as_ref() {
            let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
            if id.refresh_at > now {
                return Some(id.clone());
            }
        }

        // Otherwise, register fresh
        match self.register_server(a2s) {
            Ok(new_id) => {
                *lock = Some(new_id.clone());
                Some(new_id)
            },
            Err(_) => None,
        }
    }

    fn register_server(&self, a2s: &A2SClient) -> Result<Identity, ()> {
        let info = a2s.info(&self.server_addr).map_err(|e| serror!(f; "A2S Error: {}", e))?;
        let args = &globals().cli_args;
        let backend = args.server_browser_backend.clone().ok_or(())?;

        let desc = format!("{} (build {})\n{} server ", info.game, info.version, info.folder); // Restored

        let request = RegisterRequest {
            ports: Ports {
                game: info.extended_server_info.port.unwrap_or(7777),
                ping: args.game_server_ping_port.unwrap_or(0),
                a2s: args.game_server_query_port.unwrap_or(0),
            },
            name: &info.name,
            description: desc,
            password_protected: args.server_password.is_some(),
            current_map: &info.map,
            player_count: info.players as i32,
            max_players: info.max_players as i32,
            local_ip_address: "127.0.0.1",
            mods: &[],
        };

        let res = self.client.post(format!("{}/api/v1/servers", backend)).json(&request).send();
        match res {
            Ok(r) if r.status().is_success() => Ok(r.json::<RegisterResponse>().map_err(|_| ())?.into()),
            _ => Err(()),
        }
    }

    fn perform_update_and_heartbeat(&self, a2s: &A2SClient, ident: &Identity) -> Result<(), bool> {
        let info = a2s.info(&self.server_addr).map_err(|_| false)?;
        let backend = globals().cli_args.server_browser_backend.clone().unwrap_or_default();

        // 1. PUT Update
        let payload = UpdatePayload {
            player_count: info.players as i32,
            max_players: info.max_players as i32,
            map_name: &info.map,
        };
        let upd = self.client.put(format!("{}/api/v1/servers/{}", backend, ident.id))
            .header("X-CHIV2-SERVER-BROWSER-KEY", &ident.key)
            .json(&payload).send();

        if let Ok(r) = upd {
            if r.status().as_u16() == 401 || r.status().as_u16() == 403 { return Err(true); }
        }

        // 2. POST Heartbeat
        let hb = self.client.post(format!("{}/api/v1/servers/{}/heartbeat", backend, ident.id))
            .header("X-CHIV2-SERVER-BROWSER-KEY", &ident.key).send();

        match hb {
            Ok(r) if r.status().is_success() => Ok(()),
            Ok(r) if r.status().as_u16() == 401 || r.status().as_u16() == 403 => Err(true),
            _ => Err(false),
        }
    }

    fn is_stopping(&self) -> bool {
        *self.state.lock().unwrap() == WorkerState::StopRequested
    }
}