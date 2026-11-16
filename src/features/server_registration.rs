use crate::events::models::{GameEvent, ServerStatus};
use crate::features::events::EVENT_SYSTEM;
use crate::features::Mod;
use a2s::info::Info;
use server_browser_client::models::{
    Chivalry2Ports, ServerRegistrationRequest, UpdateRegisteredServer,
};
use server_browser_client_sdk::{
    Event, HttpTransport, Policy, Registration as SdkRegistration, SdkError,
};
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Duration;
use tokio::{sync::Notify, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::features::tokio_runtime::TOKIO_RUNTIME;

use crate::globals;
use crate::tools::hook_globals::cli_args;

#[path = "a2s_query.rs"]
mod a2s_query;

const METADATA_POLL_INTERVAL: Duration = Duration::from_secs(5);
const A2S_MAX_ATTEMPTS: NonZeroUsize = NonZeroUsize::new(3).unwrap();
// Allow the bounded in-flight HTTP request and one DELETE to finish.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(25);

pub async fn shutdown() {
    let registration = globals()
        .registration
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    if let Some(mut registration) = registration {
        registration.shutdown.cancel();
        match tokio::time::timeout(SHUTDOWN_TIMEOUT, &mut registration.task).await {
            Ok(Ok(())) => sinfo!("Server-browser registration task stopped"),
            Ok(Err(error)) => swarn!("Server-browser registration task failed: {}", error),
            Err(_) => swarn!("Server-browser registration shutdown timed out"),
        }
    }
}

/// Manages the background lifecycle of a game server registration.
#[derive(Debug)]
pub struct Registration {
    shutdown: CancellationToken,
    task: JoinHandle<()>,
    update: Arc<Notify>,
}

#[derive(Debug)]
struct RegistrationWorker {
    query_address: SocketAddr,
    local_ip_address: Option<String>,
    backend: HttpTransport,
    registration: Option<SdkRegistration<HttpTransport>>,
}

impl Registration {
    /// Initializes the registration manager.
    pub fn new(query_address: SocketAddr, local_ip: Option<&str>) -> Self {
        let advertised_local_ip = local_ip
            .filter(|address| !address.is_empty())
            .map(str::to_string);

        if let Some(address) = advertised_local_ip.as_deref() {
            sinfo!(
                "Advertising server-browser registration with local address {}",
                address
            );
        }

        let backend = crate::backend_config::client().clone();

        let worker = RegistrationWorker {
            query_address,
            local_ip_address: advertised_local_ip,
            backend,
            registration: None,
        };
        let shutdown = CancellationToken::new();
        let update = Arc::new(Notify::new());
        let task = TOKIO_RUNTIME.spawn(worker.run(shutdown.clone(), Arc::clone(&update)));
        Self {
            shutdown,
            task,
            update,
        }
    }

    /// Forces the worker to wake up and perform an update (e.g., on map change).
    pub fn trigger_update(&self) {
        self.update.notify_one();
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        // Let in-flight HTTP finish before cleanup; aborting could lose a new identity.
        self.shutdown.cancel();
    }
}

impl RegistrationWorker {
    async fn run(mut self, shutdown: CancellationToken, update: Arc<Notify>) {
        let mut poll_at = tokio::time::Instant::now();
        loop {
            // Cancellation is observed between requests, never during a registration POST.
            tokio::select! {
                biased;
                _ = shutdown.cancelled() => break,
                _ = update.notified() => {},
                _ = tokio::time::sleep_until(poll_at) => {},
            }

            match self.poll(&shutdown).await {
                Ok(delay) => poll_at = tokio::time::Instant::now() + delay,
                Err(error) => {
                    // Preserve uncertainty instead of creating a fresh manager.
                    swarn!("Server-browser lifecycle halted: {}", error);
                    break;
                }
            }
        }
        if let Some(registration) = self.registration.as_mut() {
            if let Err(error) = registration.stop().await {
                swarn!("Server-browser cleanup failed: {}", error);
            }
        }
    }

    async fn poll(&mut self, shutdown: &CancellationToken) -> Result<Duration, SdkError> {
        let status = self.refresh_metadata(shutdown).await?;
        if shutdown.is_cancelled() {
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
        shutdown: &CancellationToken,
    ) -> Result<Option<ServerStatus>, SdkError> {
        let info =
            match a2s_query::query_with_retry(self.query_address, A2S_MAX_ATTEMPTS, shutdown).await {
                Ok(Some(info)) => info,
                Ok(None) => return Ok(None),
                Err(error) => {
                    serror!("A2S failed after retries: {}", error);
                    return Ok(None);
                }
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
}
