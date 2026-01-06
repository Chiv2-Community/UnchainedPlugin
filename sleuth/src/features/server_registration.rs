use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use a2s::A2SClient;

use crate::{globals, serror, sinfo, swarn};

const INITIAL_DELAY_SECS: u64 = 20;
const DEBUG_LOGGING: bool = true; // Can be moved to a config or feature flag

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
    refresh_period: u64,
}

impl From<RegisterResponse> for Identity {
    fn from(res: RegisterResponse) -> Self {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let diff = if res.refresh_before as u64 > now {
            (res.refresh_before as u64 - now) / 3
        } else {
            10 // Fallback
        };

        Identity {
            id: res.server.unique_id,
            key: res.key,
            refresh_period: diff.max(10), 
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

    /// Spawns the background registration and heartbeat thread.
    pub fn start(self: Arc<Self>) {
        let mut state_lock = self.inner.state.lock().unwrap();
        if let WorkerState::Running = *state_lock {
            swarn!(f; "Registration worker is already running.");
            return;
        }
        *state_lock = WorkerState::Running;

        let inner = Arc::clone(&self.inner);
        let mut handle_lock = self.handle.lock().unwrap();
        
        *handle_lock = Some(thread::spawn(move || {
            if DEBUG_LOGGING { sinfo!(f; "Background worker started."); }
            
            // 1. Initial required delay
            let (lock, cvar) = (&inner.state, &inner.cvar);
            let mut state = lock.lock().unwrap();
            let _ = cvar.wait_timeout(state, Duration::from_secs(INITIAL_DELAY_SECS)).unwrap();
            
            inner.run_loop();
        }));
    }

    /// Signals the heartbeat to happen immediately (e.g., on map change).
    pub fn trigger_update(&self) {
        if DEBUG_LOGGING { sinfo!(f; "Triggering immediate update/heartbeat."); }
        self.inner.cvar.notify_all();
    }

    /// Stops the background thread and performs deregistration.
    pub fn stop(&self) {
        {
            let mut state = self.inner.state.lock().unwrap();
            *state = WorkerState::StopRequested;
        }
        self.inner.cvar.notify_all();

        if let Some(handle) = self.handle.lock().unwrap().take() {
            let _ = handle.join();
        }
        
        self.deregister();
    }

    fn deregister(&self) {
        let ident_lock = self.inner.identity.lock().unwrap();
        if let Some(ident) = ident_lock.as_ref() {
            let backend = globals().cli_args.server_browser_backend.clone().unwrap_or_default();
            let url = format!("{}/api/v1/servers/{}", backend, ident.id);
            
            match self.inner.client.delete(url)
                .header("X-CHIV2-SERVER-BROWSER-KEY", &ident.key)
                .send() 
            {
                Ok(_) => sinfo!(f; "Successfully deregistered server."),
                Err(e) => serror!(f; "Failed to deregister: {}", e),
            }
        }
    }
}

impl RegistrationInner {
    fn run_loop(&self) {
        let a2s = A2SClient::new().unwrap();
        
        loop {
            // Check shutdown signal
            {
                let state = self.state.lock().unwrap();
                if let WorkerState::StopRequested = *state { break; }
            }

            // 1. Ensure Identity (Discovery + Registration)
            let current_ident = self.identity.lock().unwrap().clone();
            let ident = match current_ident {
                Some(i) => i,
                None => match self.perform_registration(&a2s) {
                    Ok(new_ident) => {
                        let mut lock = self.identity.lock().unwrap();
                        *lock = Some(new_ident.clone());
                        new_ident
                    }
                    Err(_) => {
                        thread::sleep(Duration::from_secs(10));
                        continue;
                    }
                }
            };

            // 2. Perform Update and Heartbeat
            if let Err(fatal) = self.perform_heartbeat(&a2s, &ident) {
                if fatal {
                    serror!(f; "Fatal authorization error. Stopping worker.");
                    let mut lock = self.identity.lock().unwrap();
                    *lock = None;
                    break; 
                }
            }

            // 3. Wait for next period or notification (map change/stop)
            let state = self.state.lock().unwrap();
            let result = self.cvar.wait_timeout(state, Duration::from_secs(ident.refresh_period)).unwrap();
            if let WorkerState::StopRequested = *result.0 { break; }
        }
    }

    fn perform_registration(&self, a2s: &A2SClient) -> Result<Identity, ()> {
        let info = a2s.info(&self.server_addr).map_err(|e| {
            serror!(f; "A2S Discovery failed: {}", e);
        })?;

        let args = &globals().cli_args;
        let backend = args.server_browser_backend.clone().ok_or(())?;
        
        let request = RegisterRequest {
            ports: Ports {
                game: info.extended_server_info.port.unwrap_or(7777),
                ping: args.game_server_ping_port.unwrap_or(0),
                a2s: args.game_server_query_port.unwrap_or(0),
            },
            name: &info.name,
            description: "Refactored Game Server", // Logic simplified for brevity
            password_protected: args.server_password.is_some(),
            current_map: &info.map,
            player_count: info.players as i32,
            max_players: info.max_players as i32,
            local_ip_address: "127.0.0.1",
            mods: &[],
        };

        let res = self.client.post(format!("{}/api/v1/servers", backend))
            .json(&request)
            .send();

        match res {
            Ok(r) if r.status().is_success() => {
                let data: RegisterResponse = r.json().map_err(|_| ())?;
                Ok(data.into())
            }
            Ok(r) => {
                serror!(f; "Registration failed with status: {}", r.status());
                Err(())
            }
            Err(e) => {
                serror!(f; "Network error during registration: {}", e);
                Err(())
            }
        }
    }

    fn perform_heartbeat(&self, a2s: &A2SClient, ident: &Identity) -> Result<(), bool> {
        let info = a2s.info(&self.server_addr).map_err(|_| false)?;
        let backend = globals().cli_args.server_browser_backend.clone().unwrap_or_default();

        let payload = UpdatePayload {
            player_count: info.players as i32,
            max_players: info.max_players as i32,
            map_name: &info.map,
        };

        // Update call
        let update_res = self.client.put(format!("{}/api/v1/servers/{}", backend, ident.id))
            .header("X-CHIV2-SERVER-BROWSER-KEY", &ident.key)
            .json(&payload)
            .send();

        if let Ok(r) = update_res {
            if r.status().as_u16() == 401 || r.status().as_u16() == 403 { return Err(true); }
        }

        // Heartbeat call
        let hb_res = self.client.post(format!("{}/api/v1/servers/{}/heartbeat", backend, ident.id))
            .header("X-CHIV2-SERVER-BROWSER-KEY", &ident.key)
            .send();

        match hb_res {
            Ok(r) if r.status().is_success() => {
                if DEBUG_LOGGING { sinfo!(f; "Heartbeat success for {}", ident.id); }
                Ok(())
            }
            Ok(r) if r.status().as_u16() == 401 || r.status().as_u16() == 403 => Err(true),
            _ => Err(false),
        }
    }
}