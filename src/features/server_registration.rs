use crate::events::models::{GameEvent, ServerStatus};
use crate::features::events::EVENT_SYSTEM;
use crate::features::Mod;
use a2s::info::Info;
use server_browser_client::models::ServerRegistrationRequest;
use server_browser_client_sdk::{
    Event, Policy, Registration as SdkRegistration, RegistrationControl,
};
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::{sync::Notify, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::features::tokio_runtime::TOKIO_RUNTIME;

use crate::globals;
use crate::tools::hook_globals::cli_args;

#[path = "a2s_query.rs"]
mod a2s_query;
#[path = "registration_metadata.rs"]
mod registration_metadata;

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
    refresh: Arc<Notify>,
}

#[derive(Debug)]
struct MetadataCollector {
    query_address: SocketAddr,
    local_ip_address: Option<String>,
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

        let collector = MetadataCollector {
            query_address,
            local_ip_address: advertised_local_ip,
        };
        let shutdown = CancellationToken::new();
        let refresh = Arc::new(Notify::new());
        let task = TOKIO_RUNTIME.spawn(collector.run(shutdown.clone(), Arc::clone(&refresh)));
        Self {
            shutdown,
            task,
            refresh,
        }
    }

    /// Refresh A2S metadata; the SDK decides whether an HTTP update is needed.
    pub fn trigger_update(&self) {
        self.refresh.notify_one();
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        // Let in-flight HTTP finish before cleanup; aborting could lose a new identity.
        self.shutdown.cancel();
    }
}

impl MetadataCollector {
    async fn run(self, shutdown: CancellationToken, refresh: Arc<Notify>) {
        let info = loop {
            if let Some(info) = self.query(&shutdown).await {
                break info;
            }
            if !a2s_query::should_refresh(&shutdown, &refresh, METADATA_POLL_INTERVAL).await {
                return;
            }
        };
        if shutdown.is_cancelled() {
            return;
        }
        let status = Self::metadata(&info);
        let mut registration = match SdkRegistration::new(
            crate::backend_config::client().clone(),
            self.request(&info, &status),
            Policy::default(),
        ) {
            Ok(registration) => registration,
            Err(error) => {
                swarn!("Server-browser lifecycle halted: {}", error);
                return;
            }
        };
        let control = RegistrationControl::default();
        let latest = Mutex::new(status);
        let result = {
            let driver = registration.run(&control, |event| Self::observe(event, &latest));
            tokio::pin!(driver);
            tokio::select! {
                biased;
                () = self.collect_updates(&control, &latest, &shutdown, &refresh) => {
                    control.cancel();
                    driver.await
                }
                result = &mut driver => result,
            }
        };
        if let Err(error) = result {
            swarn!("Server-browser lifecycle halted: {}", error);
            if let Err(error) = registration.stop().await {
                swarn!("Server-browser cleanup failed: {}", error);
            }
        }
    }

    async fn query(&self, shutdown: &CancellationToken) -> Option<Info> {
        match a2s_query::query_with_retry(self.query_address, A2S_MAX_ATTEMPTS, shutdown).await {
            Ok(info) => info,
            Err(error) => {
                serror!("A2S failed after retries: {}", error);
                None
            }
        }
    }

    async fn collect_updates(
        &self,
        control: &RegistrationControl,
        latest: &Mutex<ServerStatus>,
        shutdown: &CancellationToken,
        refresh: &Notify,
    ) {
        while a2s_query::should_refresh(shutdown, refresh, METADATA_POLL_INTERVAL).await {
            let Some(info) = self.query(shutdown).await else {
                continue;
            };
            let status = Self::metadata(&info);
            let update = registration_metadata::update(&status);
            *latest
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = status;
            if !control.publish(update) {
                return;
            }
        }
    }

    fn request(&self, info: &Info, status: &ServerStatus) -> ServerRegistrationRequest {
        let args = cli_args();
        let ports = registration_metadata::ports(
            info.extended_server_info.port,
            args.game_port,
            args.game_server_ping_port,
            self.query_address.port(),
        );
        registration_metadata::request(status, ports, self.local_ip_address.clone())
    }

    fn observe(event: &Event, latest: &Mutex<ServerStatus>) {
        match event {
            Event::Registered { server_id } => {
                sinfo!("Registered server-browser identity: {}", server_id);
                let status = latest
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone();
                EVENT_SYSTEM
                    .game_event_publisher
                    .publish(GameEvent::ServerStatusEvent(status));
            }
            Event::Heartbeat => sdebug!("Server-browser heartbeat succeeded"),
            Event::Updated => sdebug!("Server-browser metadata updated"),
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

        let (player_count, max_players) =
            registration_metadata::player_counts(info.players, info.max_players, args.is_headless);
        ServerStatus {
            name: args
                .find_ini_value(&[("Game", "[/Script/TBL.TBLGameMode]", "ServerName")])
                .unwrap_or(&info.name)
                .to_owned(),
            description,
            password_protected: args.server_password.is_some(),
            current_map: info.map.clone(),
            player_count,
            max_players,
            mods: all_mods,
            active_mods: mods,
        }
    }
}
