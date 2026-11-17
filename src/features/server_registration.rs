use crate::events::models::{GameEvent, ServerStatus};
use crate::features::events::EVENT_SYSTEM;
use crate::features::Mod;
use a2s::info::Info;
use server_browser_client::models::ServerRegistrationRequest;
use server_browser_client_sdk::{
    Event, HttpEvent, HttpTransport, LifecycleEvent, ObserverResult, Policy,
    Registration as SdkRegistration, RegistrationWorker, RequestFailure,
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
        let registration = match SdkRegistration::new(
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
        let registration = registration.into_worker();
        let latest = Mutex::new(status);
        let driver = registration.run(|event| Self::reg_listeners(event, &latest));
        tokio::pin!(driver);
        let result = tokio::select! {
            biased;
            () = self.collect_updates(&registration, &latest, &shutdown, &refresh) => {
                registration.cancel();
                driver.await
            }
            result = &mut driver => result,
        };
        if let Err(error) = result {
            swarn!("Server-browser lifecycle halted: {}", error);
        }
    }

    fn reg_listeners(event: &Event<'_>, latest: &Mutex<ServerStatus>) -> ObserverResult {
        match event {
            Event::Lifecycle(event) => Self::observe(event, latest),
            Event::Http(HttpEvent::Register(response)) => match response {
                Ok(response) => sinfo!(
                    "Registered server-browser identity: {}",
                    response.server.unique_id
                ),
                Err(error) => swarn!(
                    "Server-browser registration failed: {:?}",
                    RequestFailure::from_error(error)
                ),
            },
            Event::Http(HttpEvent::Heartbeat(response)) => match response {
                Ok(_) => sdebug!("Server-browser heartbeat succeeded"),
                Err(error) => swarn!(
                    "Server-browser heartbeat failed: {:?}",
                    RequestFailure::from_error(error)
                ),
            },
            Event::Http(HttpEvent::Update(response)) => match response {
                Ok(_) => sdebug!("Server-browser metadata updated"),
                Err(error) => swarn!(
                    "Server-browser update failed: {:?}",
                    RequestFailure::from_error(error)
                ),
            },
            Event::Http(HttpEvent::Delete(response)) => match response {
                Ok(_) => sinfo!("Server-browser registration deleted"),
                Err(error) if RequestFailure::from_error(error) == RequestFailure::Http(404) => {
                    sinfo!("Server-browser registration already absent")
                }
                Err(error) => swarn!(
                    "Server-browser deletion failed: {:?}",
                    RequestFailure::from_error(error)
                ),
            },
        }
        Ok(())
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
        registration: &RegistrationWorker<HttpTransport>,
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
            if !registration.set_update(update) {
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

    fn observe(event: &LifecycleEvent, latest: &Mutex<ServerStatus>) {
        match event {
            LifecycleEvent::Registered { .. } => {
                let status = latest
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone();
                EVENT_SYSTEM
                    .game_event_publisher
                    .publish(GameEvent::ServerStatusEvent(status));
            }
            LifecycleEvent::RegistrationLost => {
                swarn!("Server-browser registration expired; SDK will re-register")
            }
            LifecycleEvent::RetryScheduled(delay) => {
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
