use crate::events::models::{GameEvent, ServerStatus};
use crate::features::events::EVENT_SYSTEM;
use crate::features::Mod;
use a2s::info::Info;
use get_if_addrs::{get_if_addrs, IfAddr};
use server_browser_client::models::{
    Chivalry2Ports, ServerRegistrationRequest, UpdateRegisteredServer,
};
use server_browser_client_sdk::{
    Event, HttpTransport, Policy, Registration as SdkRegistration, SdkError,
};
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Notify};

use crate::features::tokio_runtime::TOKIO_RUNTIME;

use crate::globals;
use crate::tools::hook_globals::cli_args;

#[path = "a2s_query.rs"]
mod a2s_query;
#[path = "registration_control.rs"]
mod registration_control;
use registration_control::{Command, TaskState};

const METADATA_POLL_INTERVAL: Duration = Duration::from_secs(5);
const A2S_RETRIES: u8 = 3;

/// Manages the background lifecycle of a game server registration.
#[derive(Debug)]
pub struct Registration {
    commands: mpsc::UnboundedSender<Command>,
    update: Arc<Notify>,
}

#[derive(Debug)]
struct RegistrationWorker {
    server_addr: SocketAddr,
    local_ip_address: Option<String>,
    backend: HttpTransport,
    registration: Option<SdkRegistration<HttpTransport>>,
}

fn discover_global_ipv6() -> Option<Ipv6Addr> {
    // The game may run inside a container/Wine namespace where interface
    // enumeration does not expose the host's global address. The launcher
    // therefore supplies SERVER_ADVERTISE_IP as the preferred stable source;
    // this discovery path is the fallback for native IPv6 environments.
    get_if_addrs().ok()?.into_iter().find_map(|interface| {
        let IfAddr::V6(address) = interface.addr else {
            return None;
        };
        let ip = address.ip;
        ip.is_unicast_global().then_some(ip)
    })
}

impl Registration {
    /// Initializes the registration manager.
    pub fn new(ip: IpAddr, query_port: u16, local_ip: Option<&str>, use_ipv6: bool) -> Self {
        let advertised_local_ip = local_ip
            .filter(|address| !address.is_empty())
            .map(str::to_string);
        let bind_address = if use_ipv6 {
            match local_ip {
                Some(address) => address.parse::<Ipv6Addr>().ok(),
                None => std::env::var("SERVER_ADVERTISE_IP")
                    .ok()
                    .and_then(|configured| configured.parse::<Ipv6Addr>().ok())
                    .or_else(discover_global_ipv6)
                    .or_else(|| {
                        swarn!("--use-ipv6 could not discover or obtain SERVER_ADVERTISE_IP; using the OS-selected source");
                        None
                    }),
            }
        } else {
            None
        };

        if let Some(address) = advertised_local_ip.as_deref() {
            sinfo!(
                "Advertising server-browser registration with local address {}",
                address
            );
        } else if let Some(address) = bind_address {
            sinfo!(
                "Using discovered IPv6 source {} without advertising local_ip_address",
                address
            );
        }

        if let Some(address) = bind_address {
            sinfo!(
                "Binding server-browser registration HTTP to IPv6 source {}",
                address
            );
        }
        let backend = crate::backend::transport(
            cli_args()
                .server_browser_backend
                .as_deref()
                .unwrap_or_default(),
            bind_address.map(IpAddr::V6),
        )
        .expect("Failed to construct bounded server-browser client");

        let worker = RegistrationWorker {
            server_addr: SocketAddr::new(ip, query_port),
            local_ip_address: advertised_local_ip,
            backend,
            registration: None,
        };
        let (commands, receiver) = mpsc::unbounded_channel();
        let update = Arc::new(Notify::new());
        // Closing the command channel drives cleanup; dropping the task handle
        // detaches it rather than aborting an in-flight registration request.
        drop(TOKIO_RUNTIME.spawn(worker.run(receiver, Arc::clone(&update))));
        Self { commands, update }
    }

    /// Resume the single task, retaining the paused SDK identity.
    pub fn start(&self) {
        let _ = self.commands.send(Command::Start);
    }

    /// Forces the worker to wake up and perform an update (e.g., on map change).
    pub fn trigger_update(&self) {
        self.update.notify_one();
    }

    /// Queue pause/cleanup without blocking a native hook or runtime worker.
    /// Any in-flight request completes before cleanup; cleanup is one-shot.
    pub fn stop(&self, deregister: bool) {
        let _ = self.commands.send(if deregister {
            Command::Cleanup
        } else {
            Command::Pause
        });
    }
}

impl RegistrationWorker {
    async fn run(mut self, mut commands: mpsc::UnboundedReceiver<Command>, update: Arc<Notify>) {
        let mut state = TaskState::default();
        let mut poll_at = tokio::time::Instant::now();
        loop {
            // One owner orders pause/resume/cleanup after in-flight HTTP. Never
            // cancel a registration POST and risk creating a second identity.
            tokio::select! {
                biased;
                command = commands.recv() => {
                    if state.apply(command.unwrap_or(Command::Cleanup)) {
                        self.cleanup().await;
                    }
                    if command.is_none() {
                        break;
                    }
                    poll_at = tokio::time::Instant::now();
                    continue;
                }
                _ = update.notified(), if state.active() => {},
                _ = tokio::time::sleep_until(poll_at), if state.active() => {},
            }

            match self.poll(&commands).await {
                Ok(delay) => poll_at = tokio::time::Instant::now() + delay,
                Err(error) => {
                    // Preserve uncertainty instead of creating a fresh manager.
                    swarn!("Server-browser lifecycle halted: {}", error);
                    state.halt();
                }
            }
        }
    }

    async fn poll(
        &mut self,
        commands: &mpsc::UnboundedReceiver<Command>,
    ) -> Result<Duration, SdkError> {
        let status = self.refresh_metadata(commands).await?;
        // A pause/cleanup arriving during A2S must precede new HTTP.
        if !commands.is_empty() || commands.is_closed() {
            return Ok(Duration::ZERO);
        }
        if let Some(registration) = self.registration.as_mut() {
            Self::observe(registration.poll().await?, status);
        }
        Ok(self
            .registration
            .as_ref()
            .and_then(SdkRegistration::next_delay)
            .unwrap_or(METADATA_POLL_INTERVAL)
            .min(METADATA_POLL_INTERVAL))
    }

    async fn refresh_metadata(
        &mut self,
        commands: &mpsc::UnboundedReceiver<Command>,
    ) -> Result<Option<ServerStatus>, SdkError> {
        let Some(info) = self.poll_a2s_with_retry(commands).await else {
            return Ok(None);
        };
        let status = Self::metadata(&info);
        let update = UpdateRegisteredServer {
            player_count: status.player_count,
            max_players: status.max_players,
            current_map: status.current_map.clone(),
            mods: Some(
                status
                    .active_mods
                    .iter()
                    .map(|m| {
                        server_browser_client::models::Mod::new(
                            m.name.clone(),
                            m.organization.clone(),
                            m.version.clone(),
                        )
                    })
                    .collect(),
            ),
        };
        if let Some(registration) = self.registration.as_mut() {
            registration.set_update(update)?;
        } else {
            let args = cli_args();
            let request = ServerRegistrationRequest {
                ports: Chivalry2Ports {
                    game: i32::from(info.extended_server_info.port.unwrap_or(7777)),
                    ping: i32::from(args.game_server_ping_port.unwrap_or(0)),
                    a2s: i32::from(args.game_server_query_port.unwrap_or(0)),
                },
                name: status.name.clone(),
                description: status.description.clone(),
                password_protected: Some(status.password_protected),
                current_map: update.current_map,
                player_count: update.player_count,
                max_players: update.max_players,
                local_ip_address: self.local_ip_address.clone(),
                mods: update.mods,
            };
            self.registration = Some(SdkRegistration::new(
                self.backend.clone(),
                request,
                Policy::default(),
            )?);
        }
        Ok(Some(status))
    }

    fn observe(event: Event, status: Option<ServerStatus>) {
        match event {
            Event::Registered { server_id } => {
                sinfo!("Registered server-browser identity: {}", server_id);
                if let Some(status) = status {
                    EVENT_SYSTEM
                        .game_event_publisher
                        .publish(GameEvent::ServerStatusEvent(status));
                }
            }
            Event::RegistrationLost => {
                swarn!("Server-browser registration expired; SDK will re-register")
            }
            Event::RetryScheduled(delay) => {
                swarn!("Server-browser retry in {}s", delay.as_secs())
            }
            _ => {}
        }
    }

    async fn cleanup(&mut self) {
        if let Some(registration) = self.registration.as_mut() {
            if let Err(error) = registration.stop().await {
                swarn!("Server-browser cleanup failed: {}", error);
            }
        }
    }

    fn metadata(info: &Info) -> ServerStatus {
        let args = cli_args();
        let description = format!(
            "{} (build {})\n{} server",
            info.game, info.version, info.folder
        );
        #[cfg(feature = "mod_management")]
        let (mods, all_mods): (Vec<Mod>, Vec<Mod>) = {
            // The Asset Registry is scanned once during init (see `world_init`),
            // so the mod manager's registry is already populated here. Read the
            // active and available mods straight from it instead of re-running
            // the game-thread scan (and its settle sleep) on every registration.
            if let Some(mm) = globals().mod_manager.lock().unwrap().as_ref() {
                (
                    mm.get_active_mod_metadata(),
                    mm.get_available().values().cloned().collect(),
                )
            } else {
                (Vec::new(), Vec::new())
            }
        };

        #[cfg(not(feature = "mod_management"))]
        let (mods, all_mods): (Vec<Mod>, Vec<Mod>) = (Vec::new(), Vec::new());

        // The headless host is counted by A2S but is not a player.
        let headless_host = u8::from(args.is_headless);
        ServerStatus {
            name: args
                .find_ini_value(&[("Game", "[/Script/TBL.TBLGameMode]", "ServerName")])
                .unwrap_or(&info.name)
                .to_owned(),
            description,
            password_protected: args.server_password.is_some(),
            current_map: info.map.clone(),
            player_count: i32::from(info.players.saturating_sub(headless_host)),
            max_players: i32::from(info.max_players.saturating_sub(headless_host)),
            mods: all_mods,
            active_mods: mods,
        }
    }

    async fn poll_a2s_with_retry(
        &self,
        commands: &mpsc::UnboundedReceiver<Command>,
    ) -> Option<Info> {
        for i in 0..A2S_RETRIES {
            if !commands.is_empty() || commands.is_closed() {
                return None;
            }
            match a2s_query::query(self.server_addr).await {
                Ok(info) => return Some(info),
                Err(error) => {
                    if i == A2S_RETRIES - 1 {
                        serror!("A2S failed after {} attempts: {:?}", A2S_RETRIES, error);
                        return None;
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
        None
    }
}
