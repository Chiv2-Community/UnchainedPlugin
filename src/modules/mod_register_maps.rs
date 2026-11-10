
use crate::commands::Command;
use crate::events::models::{CommandRequest, CommandSource, PermissionFlags};
use async_trait::async_trait;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Clone)]
#[command(name = "registermodmaps", about = "Discovers and registers mod maps from the game's Content/Paks dir into the Asset Registry")]
pub struct RegisterModMapsArgs {
    /// Optional pak directory or single pak file; defaults to Content/Paks.
    pub path: Option<String>,
}

pub struct RegisterModMapsCommand;

#[async_trait]
impl Command<RegisterModMapsArgs> for RegisterModMapsCommand {
    fn group(&self) -> String { "Mod Management".into() }
    fn required_permissions(&self) -> PermissionFlags { PermissionFlags::ADMIN }
    // Not gated to a specific source so it also works on a client-hosted
    // listen server / in-game chat, not just a dedicated server console.
    fn required_source(&self) -> Option<CommandSource> { None }
    async fn execute(&self, args: RegisterModMapsArgs, _command: &CommandRequest) {
        use crate::features::mod_maps::{default_paks_dir, MapSource, register_mod_maps_async};

        let source = match args.path.map(PathBuf::from) {
            Some(path) if path.is_dir() => MapSource::PakDir(path),
            Some(path) => MapSource::PakFiles(vec![path]),
            None => match default_paks_dir() {
                Some(dir) => MapSource::PakDir(dir),
                None => {
                    serror!(fn; "Could not derive Content/Paks from the executable; pass a path");
                    return;
                }
            },
        };
        // Await the async completion with a tokio oneshot instead of a blocking
        // mpsc recv: execute() runs on a tokio worker, and a blocking recv() pins
        // that worker so the spawned discovery task is never polled (deadlock).
        let (tx, rx) = tokio::sync::oneshot::channel();
        register_mod_maps_async(source, None, Box::new(move |result| {
            let _ = tx.send(result);
        }));

        match rx.await {
            Ok(Ok(count)) => sinfo!(fn; "Registered {} mod package(s); run listmodmaps to verify", count),
            Ok(Err(e)) => {
                serror!(fn; "Mod map registration failed: {}", e);
            }
            Err(_) => serror!(fn; "Registration channel dropped before a result arrived"),
        }
    }
}
