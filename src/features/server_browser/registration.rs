use crate::events::models::GameEvent;
use crate::features::events::EVENT_SYSTEM;
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
    refresh: Arc<Notify>,
    active_mods: Arc<Mutex<Vec<Mod>>>,
}

#[derive(Debug)]
struct MetadataCollector {
    query_address: SocketAddr,
    local_ip_address: Option<String>,
    all_mods: Vec<Mod>,
    active_mods: Arc<Mutex<Vec<Mod>>>,
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

        let active_mods = Arc::new(Mutex::new(active_mods));
        let collector = MetadataCollector {
            query_address,
            local_ip_address: advertised_local_ip,
            all_mods,
            active_mods: Arc::clone(&active_mods),
        };
        let shutdown = CancellationToken::new();
        let refresh = Arc::new(Notify::new());
        let task = TOKIO_RUNTIME.spawn(collector.run(shutdown.clone(), Arc::clone(&refresh)));
        Self {
            shutdown,
            task,
            refresh,
            active_mods,
        }
    }

    /// Replace advertised active mods without waiting for an A2S refresh.
    pub(crate) fn set_active_mods(&self, mods: Vec<Mod>) {
        *self.active_mods
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = mods;
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
        let next_info = self.next_info(shutdown);
        tokio::pin!(next_info);
        loop {
            let mut status = tokio::select! {
                biased;
                _ = shutdown.cancelled() => return,
                _ = refresh.notified() => latest
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone(),
                info = &mut next_info => {
                    next_info.set(self.next_info(shutdown));
                    self.metadata(&info)
                }
            };
            status.active_mods = self.active_mods
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            let update = status.update_request();
            *latest
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = status;
            if !control.publish(update) {
                return;
            }
        }
    }

    async fn next_info(&self, shutdown: &CancellationToken) -> Info {
        loop {
            tokio::time::sleep(METADATA_POLL_INTERVAL).await;
            if let Some(info) = self.query(shutdown).await {
                return info;
            }
        }
    }

    fn request(&self, info: &Info, status: &ServerStatus) -> ServerRegistrationRequest {
        let args = cli_args();
        let ports = server_metadata::ports(
            info.extended_server_info.port,
            args.game_port,
            args.game_server_ping_port,
            self.query_address.port(),
        );
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

    fn metadata(&self, info: &Info) -> ServerStatus {
        let args = cli_args();
        let description = format!(
            "{} (build {})\n{} server",
            info.game, info.version, info.folder
        );
        let (player_count, max_players) =
            server_metadata::player_counts(info.players, info.max_players, args.is_headless);
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
            mods: self.all_mods.clone(),
            active_mods: self.active_mods
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
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
        let active_mods = Arc::new(Mutex::new(Vec::new()));
        let refresh = Arc::new(Notify::new());
        let shutdown = CancellationToken::new();
        let registration = Registration {
            shutdown: shutdown.clone(),
            task: tokio::spawn(std::future::pending()),
            refresh: Arc::clone(&refresh),
            active_mods: Arc::clone(&active_mods),
        };
        let collector = MetadataCollector {
            query_address: "127.0.0.1:0".parse().unwrap(),
            local_ip_address: None,
            all_mods: vec![available.clone()],
            active_mods,
        };
        let latest = Mutex::new(ServerStatus {
            name: "Server".into(),
            description: String::new(),
            password_protected: false,
            current_map: "Map".into(),
            player_count: 0,
            max_players: 64,
            mods: vec![available.clone()],
            active_mods: Vec::new(),
        });
        let control = RegistrationControl::default();
        let updates = collector.collect_updates(&control, &latest, &shutdown, &refresh);
        tokio::pin!(updates);
        let assertions = async {
            for mods in [vec![available.clone()], Vec::new()] {
                registration.set_active_mods(mods.clone());
                tokio::time::timeout(Duration::from_secs(1), async {
                    loop {
                        {
                            let status = latest.lock().unwrap();
                            if status.active_mods == mods {
                                assert_eq!(status.mods, vec![available.clone()]);
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
        assert_eq!(collector.all_mods, vec![available]);
    }
}
