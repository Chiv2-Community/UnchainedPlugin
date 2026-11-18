use crate::events::models::GameEvent;
use crate::features::events::EVENT_SYSTEM;
use a2s::info::Info;
use server_browser_client::models::{Chivalry2Ports, ServerRegistrationRequest};
use server_browser_client_sdk::{
    Event, Policy, Registration as SdkRegistration, RegistrationControl,
};
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::sync::Mutex;
use std::time::Duration;
use tokio::{sync::watch, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::features::tokio_runtime::TOKIO_RUNTIME;

use crate::globals;
use crate::tools::hook_globals::cli_args;

use super::{a2s_query, server_metadata};
use super::server_metadata::{Mod, ServerStatus};

const METADATA_POLL_INTERVAL: Duration = Duration::from_secs(5);
const A2S_MAX_ATTEMPTS: NonZeroUsize = NonZeroUsize::new(3).unwrap();
// Allow the bounded in-flight HTTP request and one DELETE to finish.
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(25);

pub(crate) async fn shutdown() {
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
pub(crate) struct Registration {
    shutdown: CancellationToken,
    task: JoinHandle<()>,
    active_mods: watch::Sender<Vec<Mod>>,
}

#[derive(Debug)]
struct MetadataCollector {
    query_address: SocketAddr,
    local_ip_address: Option<String>,
    all_mods: Vec<Mod>,
    active_mods: watch::Receiver<Vec<Mod>>,
    name: Option<String>,
    password_protected: bool,
    headless: bool,
    ports: Chivalry2Ports,
}

impl Registration {
    /// Initializes registration with a fixed available-mod list.
    pub(crate) fn new(
        query_address: SocketAddr,
        local_ip: Option<&str>,
        all_mods: Vec<Mod>,
        active_mods: Vec<Mod>,
    ) -> Self {
        let advertised_local_ip = local_ip
            .filter(|address| !address.is_empty())
            .map(str::to_string);

        if let Some(address) = advertised_local_ip.as_deref() {
            sinfo!(
                "Advertising server-browser registration with local address {}",
                address
            );
        }

        let args = cli_args();
        let (active_mods, mod_updates) = watch::channel(active_mods);
        let collector = MetadataCollector {
            query_address,
            local_ip_address: advertised_local_ip,
            all_mods,
            active_mods: mod_updates,
            name: args
                .find_ini_value(&[("Game", "[/Script/TBL.TBLGameMode]", "ServerName")])
                .map(str::to_owned),
            password_protected: args.server_password.is_some(),
            headless: args.is_headless,
            ports: server_metadata::ports(
                None,
                args.game_port,
                args.game_server_ping_port,
                query_address.port(),
            ),
        };
        let shutdown = CancellationToken::new();
        let task = TOKIO_RUNTIME.spawn(collector.run(shutdown.clone()));
        Self {
            shutdown,
            task,
            active_mods,
        }
    }

    /// Replace advertised active mods without waiting for an A2S refresh.
    pub(crate) fn set_active_mods(&self, mods: Vec<Mod>) {
        self.active_mods.send_replace(mods);
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        // Let in-flight HTTP finish before cleanup; aborting could lose a new identity.
        self.shutdown.cancel();
    }
}

impl MetadataCollector {
    async fn run(mut self, shutdown: CancellationToken) {
        let info = loop {
            if let Some(info) = Self::query(self.query_address, &shutdown).await {
                break info;
            }
            let changed = async {
                let _ = self.active_mods.changed().await;
            };
            if !a2s_query::should_refresh(&shutdown, changed, METADATA_POLL_INTERVAL).await {
                return;
            }
        };
        if shutdown.is_cancelled() {
            return;
        }
        let status = self.metadata(&info);
        let mut registration = match SdkRegistration::new(
            super::http::client().clone(),
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
                () = self.collect_updates(&control, &latest, &shutdown) => {
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

    async fn query(address: SocketAddr, shutdown: &CancellationToken) -> Option<Info> {
        match a2s_query::query_with_retry(address, A2S_MAX_ATTEMPTS, shutdown).await {
            Ok(info) => info,
            Err(error) => {
                serror!("A2S failed after retries: {}", error);
                None
            }
        }
    }

    async fn collect_updates(
        &mut self,
        control: &RegistrationControl,
        latest: &Mutex<ServerStatus>,
        shutdown: &CancellationToken,
    ) {
        let next_info = Self::next_info(self.query_address, shutdown);
        tokio::pin!(next_info);
        loop {
            let info = tokio::select! {
                biased;
                _ = shutdown.cancelled() => return,
                changed = self.active_mods.changed() => {
                    if changed.is_err() {
                        return;
                    }
                    None
                }
                info = &mut next_info => {
                    next_info.set(Self::next_info(self.query_address, shutdown));
                    Some(info)
                }
            };
            let mut status = latest
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            if let Some(info) = info {
                status.current_map = info.map;
                (status.player_count, status.max_players) =
                    server_metadata::player_counts(info.players, info.max_players, self.headless);
            }
            status.active_mods = self.active_mods.borrow_and_update().clone();
            let update = status.update_request();
            *latest
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = status;
            if !control.publish(update) {
                return;
            }
        }
    }

    async fn next_info(address: SocketAddr, shutdown: &CancellationToken) -> Info {
        loop {
            tokio::time::sleep(METADATA_POLL_INTERVAL).await;
            if let Some(info) = Self::query(address, shutdown).await {
                return info;
            }
        }
    }

    fn request(&self, info: &Info, status: &ServerStatus) -> ServerRegistrationRequest {
        let mut ports = self.ports.clone();
        if let Some(port) = info.extended_server_info.port {
            ports.game = i32::from(port);
        }
        status.registration_request(ports, self.local_ip_address.clone())
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

    fn metadata(&mut self, info: &Info) -> ServerStatus {
        let description = format!(
            "{} (build {})\n{} server",
            info.game, info.version, info.folder
        );
        let (player_count, max_players) =
            server_metadata::player_counts(info.players, info.max_players, self.headless);
        ServerStatus {
            name: self.name.clone().unwrap_or_else(|| info.name.clone()),
            description,
            password_protected: self.password_protected,
            current_map: info.map.clone(),
            player_count,
            max_players,
            all_mods: self.all_mods.clone(),
            active_mods: self.active_mods.borrow_and_update().clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn active_mod_updates_preserve_available_mods_without_a2s() {
        let available = Mod {
            name: "Available mod".into(),
            organization: "Community".into(),
            version: "1.0".into(),
            object_path: "/Game/Mod".into(),
        };
        let (active_mods, mod_updates) = watch::channel(Vec::new());
        let shutdown = CancellationToken::new();
        let registration = Registration {
            shutdown: shutdown.clone(),
            task: tokio::spawn(std::future::pending()),
            active_mods,
        };
        let mut collector = MetadataCollector {
            query_address: "127.0.0.1:0".parse().unwrap(),
            local_ip_address: None,
            all_mods: vec![available.clone()],
            active_mods: mod_updates,
            name: None,
            password_protected: false,
            headless: false,
            ports: Chivalry2Ports::new(9000, 3000, 7000),
        };
        let latest = Mutex::new(ServerStatus {
            name: "Server".into(),
            description: String::new(),
            password_protected: false,
            current_map: "Map".into(),
            player_count: 0,
            max_players: 64,
            all_mods: vec![available.clone()],
            active_mods: Vec::new(),
        });
        let control = RegistrationControl::default();
        let updates = collector.collect_updates(&control, &latest, &shutdown);
        tokio::pin!(updates);
        let assertions = async {
            for mods in [vec![available.clone()], Vec::new()] {
                registration.set_active_mods(mods.clone());
                tokio::time::timeout(Duration::from_secs(1), async {
                    loop {
                        {
                            let status = latest.lock().unwrap();
                            if status.active_mods == mods {
                                assert_eq!(status.all_mods, vec![available.clone()]);
                                break;
                            }
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .expect("Mod updates must not wait for A2S");
            }
        };
        tokio::select! {
            () = &mut updates => panic!("Collector stopped before receiving mod updates"),
            () = assertions => {}
        }
        shutdown.cancel();
        updates.await;
        registration.task.abort();
        assert_eq!(latest.lock().unwrap().all_mods, vec![available]);
    }
}
